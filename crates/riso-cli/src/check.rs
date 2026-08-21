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
            name: name.to_owned(),
            required,
            ok: true,
            detail: path.display().to_string(),
            hint: String::new(),
        },
        None => Check {
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

pub fn run(
    state_dir: &Path,
    theme_dirs: &[PathBuf],
    catalog_url: &str,
    desktop: Option<Desktop>,
) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(tool(
        "git",
        true,
        "themes install and update through it",
        "please install git with your package manager",
    ));
    checks.push(tool(
        "curl",
        true,
        "the catalog and previews are fetched with it",
        "please install curl with your package manager",
    ));
    checks.push(tool(
        "quickshell",
        false,
        "only --gui needs it; --tui works in any terminal",
        "please install quickshell for the full-screen carousel, or use --tui",
    ));

    let (desktop, how) = match desktop {
        Some(named) => (named, "named with --desktop"),
        None => (Desktop::detect(), "recognized from this session"),
    };
    checks.push(Check {
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
    });

    let themes = riso_core::catalog::installed(theme_dirs, None);
    checks.push(Check {
        name: "themes".to_owned(),
        required: false,
        ok: !themes.is_empty(),
        detail: format!("{} found", themes.len()),
        hint: if themes.is_empty() {
            "please install one: riso theme install caio".to_owned()
        } else {
            String::new()
        },
    });

    let current = state_dir.join("current/theme");
    checks.push(Check {
        name: "rendered theme".to_owned(),
        required: false,
        ok: current.join("colors.toml").is_file(),
        detail: current.display().to_string(),
        hint: if current.join("colors.toml").is_file() {
            String::new()
        } else {
            "nothing applied yet: riso theme set <name>".to_owned()
        },
    });

    let catalog_ok = std::process::Command::new("curl")
        .args(["-fsI", "--max-time", "10", catalog_url])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    checks.push(Check {
        name: "catalog".to_owned(),
        required: false,
        ok: catalog_ok,
        detail: catalog_url.to_owned(),
        hint: if catalog_ok {
            String::new()
        } else {
            "not reachable; installs from the catalog need the network".to_owned()
        },
    });

    let config_home = config_home();
    for wiring in WIRINGS {
        let Some(_) = on_path(wiring.binary) else {
            continue;
        };
        let config = config_home.join(wiring.config);
        let fragment = current.join(wiring.fragment);
        let line = wiring
            .include
            .replace("{}", &fragment.display().to_string());

        let (ok, detail, hint) = if !config.is_file() {
            (
                false,
                format!("installed, but {} does not exist", config.display()),
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
        checks.push(Check {
            name: wiring.app.to_owned(),
            required: false,
            ok,
            detail,
            hint,
        });
    }

    checks
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

    for check in checks {
        let mark = if check.ok { good } else { bad };
        println!("{mark} {:<16} {}", check.name, check.detail);
        if !check.hint.is_empty() {
            for line in check.hint.lines() {
                println!("  {dim}{line}{reset}");
            }
        }
    }
}
