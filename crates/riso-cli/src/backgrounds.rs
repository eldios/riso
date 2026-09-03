//! `riso backgrounds`: the current-background link, its mode, and the
//! desktop that draws it.

use std::path::{Path, PathBuf};

use riso_core::background;
use riso_core::desktop::Desktop;
use riso_core::reload::ProcessExecutor;

use crate::cli::BgAction;
use crate::output::{emit, OutputFormat};
use crate::paths::{omarchy_shell_config, state_or_default};
use crate::{data, gui, tui};

pub(crate) fn run(action: BgAction, output: OutputFormat) -> Result<(), String> {
    match action {
        BgAction::Set {
            image,
            gui,
            tui,
            state,
            no_reload,
        } => set(image, gui, tui, state, no_reload, output),
        BgAction::Next { state, no_reload } => next(state, no_reload, output),
        BgAction::Mode { mode, state } => set_mode(mode, state, output),
        BgAction::Get { state } => get(state, output),
    }
}

fn set(
    image: Option<PathBuf>,
    gui: bool,
    tui: bool,
    state: Option<PathBuf>,
    no_reload: bool,
    output: OutputFormat,
) -> Result<(), String> {
    if gui {
        return gui::run(data::What::Backgrounds, data::Purpose::Apply);
    }
    if tui {
        return tui::run(data::What::Backgrounds, data::Purpose::Apply, state);
    }
    let Some(image) = image else {
        return Err("an image is needed, or --gui/--tui to pick one".to_owned());
    };
    let state = state_or_default(state)?;
    set_background(&image, &state, no_reload, output)
}

fn next(state: Option<PathBuf>, no_reload: bool, output: OutputFormat) -> Result<(), String> {
    let state = state_or_default(state)?;
    let images = background::candidates(&[state.join("current/theme/backgrounds")]);
    let showing = background::current(&state.join("current/background"));
    let Some(chosen) = background::next(&images, showing.as_deref()) else {
        return Err("the current theme ships no backgrounds".to_owned());
    };
    set_background(&chosen, &state, no_reload, output)
}

fn set_mode(
    mode: Option<String>,
    state: Option<PathBuf>,
    output: OutputFormat,
) -> Result<(), String> {
    let state = state_or_default(state)?;
    let path = state.join("current/background.mode");
    let mode = match mode {
        Some(mode) => {
            riso_core::atomic::write_atomic(&path, &format!("{mode}\n"))
                .map_err(|e| e.to_string())?;
            if !emit(output, &serde_json::json!({ "mode": mode }))? {
                println!("mode {mode}");
            }
            return Ok(());
        }
        None => read_mode(&state),
    };
    if !emit(output, &serde_json::json!({ "mode": mode }))? {
        println!("{mode}");
    }
    Ok(())
}

fn get(state: Option<PathBuf>, output: OutputFormat) -> Result<(), String> {
    let state = state_or_default(state)?;
    let image = background::current(&state.join("current/background"));
    let mode = read_mode(&state);
    if !emit(output, &serde_json::json!({ "image": image, "mode": mode }))? {
        match &image {
            Some(path) => println!("{}", path.display()),
            None => println!("none"),
        }
        println!("mode {mode}");
    }
    Ok(())
}

/// The scaling mode on record, `fill` when nothing was ever chosen.
fn read_mode(state: &Path) -> String {
    let mode = std::fs::read_to_string(state.join("current/background.mode")).unwrap_or_default();
    let mode = mode.trim();
    if mode.is_empty() {
        "fill".to_owned()
    } else {
        mode.to_owned()
    }
}

/// Point the current-background link at `image` and tell the desktop
/// that draws its own wallpaper.
pub(crate) fn set_background(
    image: &Path,
    state: &Path,
    no_reload: bool,
    output: OutputFormat,
) -> Result<(), String> {
    let image = std::fs::canonicalize(image).map_err(|e| format!("{}: {e}", image.display()))?;
    if !image.is_file() {
        return Err(format!("{}: not a file", image.display()));
    }
    background::link(&state.join("current/background"), &image).map_err(|e| e.to_string())?;

    // Record the pick against the theme showing it, so returning to that
    // theme returns to this image instead of starting its list over.
    if let Some(theme) = data::current_theme(state) {
        let _ = background::remember(state, &theme, &image);
    }

    if !no_reload {
        Desktop::detect()
            .set_background(&ProcessExecutor, &image, omarchy_shell_config().as_deref())
            .map_err(|e| e.to_string())?;
    }
    if !emit(output, &serde_json::json!({ "background": image }))? {
        println!("background {}", image.display());
    }
    Ok(())
}
