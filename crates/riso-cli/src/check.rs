//! Checking that this system can carry riso: the tools it calls, the
//! desktop it talks to, and the include lines that let applications read
//! what it renders. Every failing line says how to fix it, because the
//! usual symptom of a missing include is silence, not an error.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use serde::Serialize;

use riso_core::desktop::Desktop;

#[derive(Debug, Serialize)]
pub struct Check {
    pub section: String,
    pub name: String,
    pub required: bool,
    pub ok: bool,
    pub detail: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hint: String,
}

/// An application riso renders for, and the line its config needs to
/// carry so the rendered fragment is actually read.
struct Wiring {
    app: &'static str,
    binary: &'static str,
    config: &'static str,
    fragment: &'static str,
    include: &'static str,
}

/// `{}` marks where the fragment's absolute path lands.
const WIRINGS: &[Wiring] = &[
    Wiring {
        app: "alacritty",
        binary: "alacritty",
        config: "alacritty/alacritty.toml",
        fragment: "alacritty.toml",
        include: "[general]\nimport = [\"{}\"]",
    },
    Wiring {
        app: "kitty",
        binary: "kitty",
        config: "kitty/kitty.conf",
        fragment: "kitty.conf",
        include: "include {}",
    },
    Wiring {
        app: "ghostty",
        binary: "ghostty",
        config: "ghostty/config",
        fragment: "ghostty.conf",
        include: "config-file = {}",
    },
    Wiring {
        app: "foot",
        binary: "foot",
        config: "foot/foot.ini",
        fragment: "foot.ini",
        include: "[main]\ninclude={}",
    },
    Wiring {
        app: "mako",
        binary: "mako",
        config: "mako/config",
        fragment: "mako.ini",
        include: "include={}",
    },
    Wiring {
        app: "waybar",
        binary: "waybar",
        config: "waybar/style.css",
        fragment: "waybar.css",
        include: "@import \"{}\";",
    },
    Wiring {
        app: "hyprland",
        binary: "Hyprland",
        config: "hypr/hyprland.conf",
        fragment: "hyprland.conf",
        include: "source = {}",
    },
    Wiring {
        app: "hyprlock",
        binary: "hyprlock",
        config: "hypr/hyprlock.conf",
        fragment: "hyprlock.conf",
        include: "source = {}",
    },
    Wiring {
        app: "btop",
        binary: "btop",
        config: "btop/btop.conf",
        fragment: "btop.theme",
        include: "color_theme = \"{}\"",
    },
];

fn on_path(binary: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

fn tool(name: &str, required: bool, purpose: &str, remedy: &str) -> Check {
    match on_path(name) {
        Some(path) => Check {
            section: "tools".to_owned(),
            name: name.to_owned(),
            required,
            ok: true,
            detail: path.display().to_string(),
            hint: String::new(),
        },
        None => Check {
            section: "tools".to_owned(),
            name: name.to_owned(),
            required,
            ok: false,
            detail: format!("not available; {purpose}"),
            hint: remedy.to_owned(),
        },
    }
}

/// Whether `config` already reads riso's tree, by the one thing every
/// include spelling shares: the path it points at.
fn includes_riso(config: &Path) -> bool {
    std::fs::read_to_string(config)
        .map(|text| text.contains("/riso/"))
        .unwrap_or(false)
}

/// One check, picked by name: a tool, a section, or an application,
/// shown whether or not the application is installed, because asking by
/// name means wanting the answer anyway.
pub fn select(
    name: &str,
    state_dir: &Path,
    theme_dirs: &[PathBuf],
    catalog_url: &str,
    desktop: Option<Desktop>,
) -> Result<Vec<Check>, String> {
    let current = state_dir.join("current/theme");
    let found = match name {
        "git" => git_check(),
        "curl" => curl_check(),
        "quickshell" => quickshell_check(),
        "desktop" => desktop_check(desktop),
        "themes" => themes_check(theme_dirs),
        "rendered" => rendered_check(&current),
        "catalog" => catalog_check(catalog_url),
        app => {
            if let Some(wiring) = WIRINGS.iter().find(|w| w.app == app) {
                let mut check = wire(wiring, &current);
                if on_path(wiring.binary).is_none() {
                    check.detail = format!("{} not found on PATH; {}", wiring.binary, check.detail);
                }
                return Ok(vec![check]);
            }
            if let Some(template) = riso_core::builtin::TEMPLATES
                .iter()
                .find(|t| t.name.split('.').next() == Some(app))
            {
                let fragment = current.join(template.name);
                return Ok(vec![Check {
                    section: "applications".to_owned(),
                    name: app.to_owned(),
                    required: false,
                    ok: true,
                    detail: format!("rendered as {}", fragment.display()),
                    hint: "no single include line for this application: \
                           point it at the file above by hand"
                        .to_owned(),
                }]);
            }
            let mut known = vec![
                "git",
                "curl",
                "quickshell",
                "desktop",
                "themes",
                "rendered",
                "catalog",
            ];
            known.extend(WIRINGS.iter().map(|w| w.app));
            known.extend(
                riso_core::builtin::TEMPLATES
                    .iter()
                    .filter_map(|t| t.name.split('.').next()),
            );
            known.sort_unstable();
            known.dedup();
            return Err(format!(
                "unknown check '{app}'; pick one of: {}",
                known.join(", ")
            ));
        }
    };
    Ok(vec![found])
}

fn git_check() -> Check {
    tool(
        "git",
        true,
        "themes install and update through it",
        "please install git with your package manager",
    )
}

fn curl_check() -> Check {
    tool(
        "curl",
        true,
        "the catalog and previews are fetched with it",
        "please install curl with your package manager",
    )
}

fn quickshell_check() -> Check {
    tool(
        "quickshell",
        false,
        "only --gui needs it; --tui works in any terminal",
        "please install quickshell for the full-screen carousel, or use --tui",
    )
}

fn desktop_check(desktop: Option<Desktop>) -> Check {
    let (desktop, how) = match desktop {
        Some(named) => (named, "named with --desktop"),
        None => (Desktop::detect(), "recognized from this session"),
    };
    Check {
        section: "environment".to_owned(),
        name: "desktop".to_owned(),
        required: false,
        ok: desktop != Desktop::None,
        detail: if desktop == Desktop::None {
            format!("none {how}")
        } else {
            format!("{} ({how})", desktop.name())
        },
        hint: if desktop == Desktop::None {
            "themes still render, but no desktop is told to reload; \
             rerun with --desktop <name> to check as one of omarchy, \
             hyprland, sway or niri"
                .to_owned()
        } else {
            String::new()
        },
    }
}

fn themes_check(theme_dirs: &[PathBuf]) -> Check {
    let themes = riso_core::catalog::installed(theme_dirs, None);
    Check {
        section: "environment".to_owned(),
        name: "themes".to_owned(),
        required: false,
        ok: !themes.is_empty(),
        detail: format!("{} found", themes.len()),
        hint: if themes.is_empty() {
            "please install one: riso theme install caio".to_owned()
        } else {
            String::new()
        },
    }
}

fn rendered_check(current: &Path) -> Check {
    let ok = current.join("colors.toml").is_file();
    Check {
        section: "environment".to_owned(),
        name: "rendered".to_owned(),
        required: false,
        ok,
        detail: current.display().to_string(),
        hint: if ok {
            String::new()
        } else {
            "nothing applied yet: riso theme set <name>".to_owned()
        },
    }
}

fn catalog_check(catalog_url: &str) -> Check {
    let ok = std::process::Command::new("curl")
        .args(["-fsI", "--max-time", "10", catalog_url])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    Check {
        section: "environment".to_owned(),
        name: "catalog".to_owned(),
        required: false,
        ok,
        detail: catalog_url.to_owned(),
        hint: if ok {
            String::new()
        } else {
            "not reachable; installs from the catalog need the network".to_owned()
        },
    }
}

pub fn run(
    state_dir: &Path,
    theme_dirs: &[PathBuf],
    catalog_url: &str,
    desktop: Option<Desktop>,
) -> Vec<Check> {
    let current = state_dir.join("current/theme");
    let resolved = desktop.unwrap_or_else(Desktop::detect);
    let mut checks = vec![
        git_check(),
        curl_check(),
        quickshell_check(),
        desktop_check(desktop),
        themes_check(theme_dirs),
        rendered_check(&current),
        catalog_check(catalog_url),
    ];

    // On Omarchy the desktop's own configuration chain reads the rendered
    // theme; per-application includes are its business, not the user's.
    if resolved == Desktop::Omarchy {
        checks.push(Check {
            section: "applications".to_owned(),
            name: "wiring".to_owned(),
            required: false,
            ok: true,
            detail: "handled by omarchy: its configs read the rendered theme".to_owned(),
            hint: String::new(),
        });
        return checks;
    }

    let mut skipped = Vec::new();
    for wiring in WIRINGS {
        if on_path(wiring.binary).is_none() {
            skipped.push(wiring.app);
            continue;
        }
        checks.push(wire(wiring, &current));
    }
    if !skipped.is_empty() {
        checks.push(Check {
            section: "applications".to_owned(),
            name: "not installed".to_owned(),
            required: false,
            ok: true,
            detail: format!("skipped: {}", skipped.join(", ")),
            hint: "name one to see its wiring anyway: riso config check foot".to_owned(),
        });
    }
    checks
}

/// One application's wiring verdict: config present, fragment included,
/// and the exact line to add when it is not.
fn wire(wiring: &Wiring, current: &Path) -> Check {
    // Hyprland 0.55+ loads hyprland.lua instead of hyprland.conf when it
    // exists, and lua cannot source a hyprlang fragment: say so rather
    // than handing out an include line the session would never read.
    if wiring.app == "hyprland" {
        let lua = config_home().join("hypr/hyprland.lua");
        if lua.is_file() {
            return Check {
                section: "applications".to_owned(),
                name: "hyprland".to_owned(),
                required: false,
                ok: false,
                detail: format!("{} is a lua config", lua.display()),
                hint: "riso's hyprland fragment is hyprlang, which a lua config \
                       cannot source; lua fragments are on riso's roadmap"
                    .to_owned(),
            };
        }
    }
    let config = config_home().join(wiring.config);
    let fragment = current.join(wiring.fragment);
    let line = wiring
        .include
        .replace("{}", &fragment.display().to_string())
        .replace('\n', "\n    ");

    let (ok, detail, hint) = if !config.is_file() {
        (
            false,
            format!("{} does not exist", config.display()),
            format!("please create it and add:\n    {line}"),
        )
    } else if includes_riso(&config) {
        (true, config.display().to_string(), String::new())
    } else {
        (
            false,
            format!("{} does not read riso's fragment", config.display()),
            format!("please add to it:\n    {line}"),
        )
    };
    Check {
        section: "applications".to_owned(),
        name: wiring.app.to_owned(),
        required: false,
        ok,
        detail,
        hint,
    }
}

/// The human rendering: a bold green V or red X per line, hints indented
/// under the lines that need them.
pub fn print(checks: &[Check]) {
    let color = std::io::stdout().is_terminal();
    let (good, bad, reset) = if color {
        ("\x1b[1;32mV\x1b[0m", "\x1b[1;31mX\x1b[0m", "\x1b[0m")
    } else {
        ("V", "X", "")
    };
    let dim = if color { "\x1b[2m" } else { "" };

    let mut section = "";
    for check in checks {
        if check.section != section {
            if !section.is_empty() {
                println!();
            }
            println!("{dim}{}:{reset}", check.section);
            section = &check.section;
        }
        let mark = if check.ok { good } else { bad };
        println!("{mark} {:<16} {}", check.name, check.detail);
        if !check.hint.is_empty() {
            for line in check.hint.lines() {
                println!("  {dim}{line}{reset}");
            }
        }
    }
}
