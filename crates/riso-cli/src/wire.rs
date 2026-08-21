//! Adding the include lines that `config check` only talks about, the
//! cautious way: a plan first, one confirmation per file, every touched
//! file captured in the ownership store so `riso restore` puts back
//! every byte, and a refusal wherever editing is not clearly safe.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::check::{config_home, effective, includes_riso, on_path, Wiring, WIRINGS};

const MARKER: &str = "added by riso config wire; riso restore puts this file back";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum Action {
    /// Safe to add at the end of the file.
    Append,
    /// Safe to add at the top of the file.
    Prepend,
    /// The file does not exist and would be created.
    Create,
    /// Nothing to do: the config already reads riso's tree.
    AlreadyWired,
    /// The config is a symlink: managed elsewhere, not ours to edit.
    Managed,
    /// Editing is not clearly safe; the line is shown instead.
    Manual,
    /// A declaratively managed system: configs are not riso's to edit.
    Declarative,
}

#[derive(Debug, Serialize)]
pub struct Plan {
    pub app: String,
    pub config: PathBuf,
    pub action: Action,
    pub text: String,
}

fn text_for(wiring: &Wiring, current: &Path) -> String {
    let fragment = current.join(wiring.fragment);
    let line = wiring
        .include
        .replace("{}", &fragment.display().to_string());
    let (open, close) = wiring.comment;
    format!("{open}{MARKER}{close}\n{line}\n")
}

/// Whether this system manages its configuration declaratively: on
/// NixOS every imperative edit is a lie the next rebuild reverts.
/// `RISO_DECLARATIVE=1|0` overrides the probe, for immutable distros
/// the probe does not know and for tests.
pub fn declarative_system() -> bool {
    match std::env::var("RISO_DECLARATIVE").as_deref() {
        Ok("1") => return true,
        Ok("0") => return false,
        _ => {}
    }
    Path::new("/etc/NIXOS").exists()
        || std::fs::read_to_string("/etc/os-release")
            .map(|s| s.lines().any(|l| l == "ID=nixos"))
            .unwrap_or(false)
}

fn plan_one(wiring: &'static Wiring, current: &Path, declarative: bool) -> Plan {
    let wiring = effective(wiring);
    let config = config_home().join(wiring.config);
    let text = text_for(wiring, current);

    if declarative && !includes_riso(&config) {
        return Plan {
            app: wiring.app.to_owned(),
            config,
            action: Action::Declarative,
            text,
        };
    }

    let action = match std::fs::symlink_metadata(&config) {
        Err(_) => Action::Create,
        Ok(_) if includes_riso(&config) => Action::AlreadyWired,
        Ok(meta) if meta.file_type().is_symlink() => Action::Managed,
        Ok(_) => {
            let content = std::fs::read_to_string(&config).unwrap_or_default();
            match wiring.conflict {
                Some(token) if content.contains(token) => Action::Manual,
                _ if wiring.prepend => Action::Prepend,
                _ => Action::Append,
            }
        }
    };

    Plan {
        app: wiring.app.to_owned(),
        config,
        action,
        text,
    }
}

/// The plan: what would be written where, and where riso keeps its hands
/// off. Naming apps plans them even when not installed.
pub fn plan(state_dir: &Path, apps: &[String], declarative: bool) -> Result<Vec<Plan>, String> {
    let current = state_dir.join("current/theme");
    let mut plans = Vec::new();

    if apps.is_empty() {
        for wiring in WIRINGS {
            if on_path(wiring.binary).is_none() {
                continue;
            }
            plans.push(plan_one(wiring, &current, declarative));
        }
    } else {
        for app in apps {
            let wiring = WIRINGS.iter().find(|w| w.app == app).ok_or_else(|| {
                let known: Vec<&str> = WIRINGS.iter().map(|w| w.app).collect();
                format!("'{app}' is not wirable; pick one of: {}", known.join(", "))
            })?;
            plans.push(plan_one(wiring, &current, declarative));
        }
    }
    Ok(plans)
}

/// Write one planned change, capturing the file first so restore can
/// undo it. Only Append, Prepend and Create ever reach this.
pub fn apply(plan: &Plan, store: &mut riso_core::snapshot::Store) -> Result<(), String> {
    store.capture(&plan.config).map_err(|e| e.to_string())?;

    let existing = std::fs::read_to_string(&plan.config).unwrap_or_default();
    let written = match plan.action {
        Action::Prepend => format!("{}{existing}", plan.text),
        Action::Append if existing.is_empty() || existing.ends_with('\n') => {
            format!("{existing}{}", plan.text)
        }
        Action::Append => format!("{existing}\n{}", plan.text),
        Action::Create => plan.text.clone(),
        _ => return Err(format!("{}: not an applicable action", plan.app)),
    };

    if let Some(parent) = plan.config.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    riso_core::atomic::write_atomic(&plan.config, &written).map_err(|e| e.to_string())
}

pub fn describe(plan: &Plan) -> String {
    match plan.action {
        Action::Append => format!("{}: append to {}", plan.app, plan.config.display()),
        Action::Prepend => format!("{}: prepend to {}", plan.app, plan.config.display()),
        Action::Create => format!("{}: create {}", plan.app, plan.config.display()),
        Action::AlreadyWired => format!("{}: already wired ({})", plan.app, plan.config.display()),
        Action::Managed => format!(
            "{}: {} is a symlink, managed elsewhere; add the line to its source instead",
            plan.app,
            plan.config.display()
        ),
        Action::Manual => format!(
            "{}: {} needs a hand-placed line (automatic editing could clash); add:",
            plan.app,
            plan.config.display()
        ),
        Action::Declarative => format!(
            "{}: declarative system, nothing is edited; carry this into your configuration:",
            plan.app
        ),
    }
}

/// Whether this plan is something `apply` can carry out.
pub fn actionable(plan: &Plan) -> bool {
    matches!(
        plan.action,
        Action::Append | Action::Prepend | Action::Create
    )
}

pub fn confirm(prompt: &str) -> bool {
    print!("{prompt} [y/N] ");
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    matches!(answer.trim(), "y" | "Y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_home<T>(dir: &Path, run: impl FnOnce() -> T) -> T {
        // Env vars are process-global and tests run in parallel.
        let _hold = ENV_LOCK.lock().unwrap();
        let saved = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", dir);
        let out = run();
        match saved {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        out
    }

    fn wiring(app: &str) -> &'static Wiring {
        WIRINGS.iter().find(|w| w.app == app).expect("known app")
    }

    #[test]
    fn a_missing_config_plans_a_create() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("kitty"), Path::new("/s"), false)
        });
        assert_eq!(plan.action, Action::Create);
        assert!(plan.text.contains("include /s/kitty.conf"));
        assert!(plan.text.contains(MARKER));
    }

    #[test]
    fn an_existing_include_plans_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("kitty")).expect("mkdir");
        std::fs::write(
            dir.path().join("kitty/kitty.conf"),
            "include /x/riso/current/theme/kitty.conf\n",
        )
        .expect("write");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("kitty"), Path::new("/s"), false)
        });
        assert_eq!(plan.action, Action::AlreadyWired);
    }

    #[test]
    fn a_conflicting_section_falls_back_to_manual() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("alacritty")).expect("mkdir");
        std::fs::write(
            dir.path().join("alacritty/alacritty.toml"),
            "[general]\nlive_config_reload = true\n",
        )
        .expect("write");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("alacritty"), Path::new("/s"), false)
        });
        assert_eq!(plan.action, Action::Manual);
    }

    #[test]
    fn a_symlinked_config_is_left_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("kitty")).expect("mkdir");
        std::fs::write(dir.path().join("real.conf"), "font_size 12\n").expect("write");
        std::os::unix::fs::symlink(
            dir.path().join("real.conf"),
            dir.path().join("kitty/kitty.conf"),
        )
        .expect("symlink");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("kitty"), Path::new("/s"), false)
        });
        assert_eq!(plan.action, Action::Managed);
    }

    #[test]
    fn a_lua_hyprland_config_gets_the_dofile_wiring() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("hypr")).expect("mkdir");
        std::fs::write(dir.path().join("hypr/hyprland.lua"), "-- mine\n").expect("write");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("hyprland"), Path::new("/s"), false)
        });
        assert_eq!(plan.action, Action::Append);
        assert!(plan.config.ends_with("hypr/hyprland.lua"));
        assert!(plan
            .text
            .contains("pcall(dofile, \"/s/hyprland.lua\")\nif err then print(err) end"));
        assert!(plan.text.starts_with("-- "));
    }

    #[test]
    fn a_hyprlang_hyprland_config_keeps_the_source_wiring() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("hypr")).expect("mkdir");
        std::fs::write(dir.path().join("hypr/hyprland.conf"), "# mine\n").expect("write");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("hyprland"), Path::new("/s"), false)
        });
        assert_eq!(plan.action, Action::Append);
        assert!(plan.config.ends_with("hypr/hyprland.conf"));
        assert!(plan.text.contains("source = /s/hyprland.conf"));
    }

    #[test]
    fn a_declarative_system_refuses_every_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("kitty")).expect("mkdir");
        std::fs::write(dir.path().join("kitty/kitty.conf"), "font_size 12\n").expect("write");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("kitty"), Path::new("/s"), true)
        });
        assert_eq!(plan.action, Action::Declarative);
        // A config already wired stays reported as such even there.
        std::fs::write(
            dir.path().join("kitty/kitty.conf"),
            "include /x/riso/current/theme/kitty.conf\n",
        )
        .expect("write");
        let plan = with_home(dir.path(), || {
            plan_one(wiring("kitty"), Path::new("/s"), true)
        });
        assert_eq!(plan.action, Action::AlreadyWired);
    }

    #[test]
    fn apply_appends_prepends_and_restores() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store_dir = dir.path().join("ownership");
        let mut store = riso_core::snapshot::Store::open(&store_dir).expect("store");

        let config = dir.path().join("waybar/style.css");
        std::fs::create_dir_all(config.parent().unwrap()).expect("mkdir");
        std::fs::write(&config, "window { color: red; }\n").expect("write");

        let plan = Plan {
            app: "waybar".to_owned(),
            config: config.clone(),
            action: Action::Prepend,
            text: "/* marker */\n@import \"x\";\n".to_owned(),
        };
        apply(&plan, &mut store).expect("apply");
        let written = std::fs::read_to_string(&config).expect("read");
        assert!(written.starts_with("/* marker */"));
        assert!(written.ends_with("red; }\n"));

        store.restore(&config).expect("restore");
        assert_eq!(
            std::fs::read_to_string(&config).expect("read"),
            "window { color: red; }\n"
        );
    }
}
