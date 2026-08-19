//! The GUI picker: the carousel, a Quickshell strip of previews.
//!
//! The QML ships inside the binary and is written out where the session can
//! read it; every hook points back at this same executable. The apply hooks
//! can be overridden through the environment, which is how a machine's own
//! wrapper takes over the handoff to whatever shells it runs.

use std::path::PathBuf;

use crate::data::What;

const CAROUSEL_QML: &str = include_str!("../../../carousel/shell.qml");

pub fn run(what: What) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();

    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("riso-carousel");
    riso_core::atomic::write_atomic(&base.join("shell.qml"), CAROUSEL_QML)
        .map_err(|e| e.to_string())?;

    let (data, apply_env, default_apply, current) = match what {
        What::Backgrounds => (
            "backgrounds",
            "RISO_CAROUSEL_APPLY_BG",
            format!("{exe} backgrounds set"),
            "readlink -f \"${XDG_STATE_HOME:-$HOME/.local/state}/riso/current/background\""
                .to_owned(),
        ),
        What::Themes => (
            "themes",
            "RISO_CAROUSEL_APPLY",
            format!("{exe} theme set"),
            "cat \"${XDG_STATE_HOME:-$HOME/.local/state}/riso/current/theme.name\"".to_owned(),
        ),
    };
    let apply = std::env::var(apply_env).unwrap_or(default_apply);

    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new("quickshell")
        .arg("-n")
        .arg("-p")
        .arg(&base)
        .env("RISO_CAROUSEL_LIST", format!("{exe} carousel-data {data}"))
        .env("RISO_CAROUSEL_APPLY", apply)
        .env("RISO_CAROUSEL_CURRENT", current)
        .exec();
    Err(format!("could not run quickshell: {error}"))
}
