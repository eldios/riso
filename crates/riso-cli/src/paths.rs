//! Where riso keeps and looks for things when no flag says otherwise.

use std::path::PathBuf;

use riso_core::catalog;

/// Where `theme install` fetches from, and the first place `set` looks.
pub(crate) const DEFAULT_CATALOG: &str = "https://catalog.riso.re/index.json";

pub(crate) fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME is not set".to_owned())
}

pub(crate) fn user_plugin_dir() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("riso/plugins"));
        }
    }
    Ok(home_dir()?.join(".config/riso/plugins"))
}

pub(crate) fn user_theme_dir() -> Result<PathBuf, String> {
    catalog::user_theme_dir().ok_or_else(|| "HOME is not set".to_owned())
}

/// Where generated theme state lives, honouring XDG before falling back.
///
/// On an Omarchy system the desktop reads its state from its own directory,
/// so riso renders where that desktop looks and is a drop-in; anywhere else
/// the state is riso's own. `--state` overrides either way.
pub(crate) fn default_state_dir() -> Result<PathBuf, String> {
    let name = if omarchy_path().is_some() {
        "omarchy"
    } else {
        "riso"
    };
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join(name));
        }
    }
    Ok(home_dir()?.join(".local/state").join(name))
}

pub(crate) fn state_or_default(state: Option<PathBuf>) -> Result<PathBuf, String> {
    match state {
        Some(dir) => Ok(dir),
        None => default_state_dir(),
    }
}

/// The Omarchy install, when its environment announces one.
pub(crate) fn omarchy_path() -> Option<PathBuf> {
    std::env::var_os("OMARCHY_PATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Omarchy publishes its shell config; with it riso calls Quickshell
/// directly rather than the wrapper script.
pub(crate) fn omarchy_shell_config() -> Option<PathBuf> {
    omarchy_path()
        .map(|p| p.join("shell"))
        .filter(|p| p.is_dir())
}
