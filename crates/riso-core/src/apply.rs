//! Applying a theme: from a name to the files on disk, and a live desktop.
//!
//! The step order is load-bearing. Rendering happens in a staging directory
//! and only a finished staging directory is swapped in, so a reader never sees
//! a half-written theme, and a failed render leaves the previous one in place.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::error::IoError;
use crate::palette::Warning;
use crate::reload::{base64, notify_omarchy_shell, Executor, ReloadError};
use crate::theme::{load_palette, render_theme, Report};

const PALETTE_FILE: &str = "colors.toml";
const SHELL_FILE: &str = "shell.toml";
const CURRENT_DIR: &str = "current";
const THEME_DIR: &str = "theme";
const THEME_NAME_FILE: &str = "theme.name";

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("no theme named '{0}' in any theme directory")]
    NotFound(String),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Reload(#[from] ReloadError),
}

#[derive(Debug, Clone)]
pub struct Request {
    /// Theme name as the user typed it; it is normalized before lookup.
    pub name: String,
    /// Theme directories, weakest first: a later one overlays an earlier one.
    pub theme_dirs: Vec<PathBuf>,
    /// Template directories, strongest first.
    pub template_dirs: Vec<PathBuf>,
    /// Where the generated theme lives, typically ~/.local/state/omarchy.
    pub state_dir: PathBuf,
    /// Skip telling the running desktop about the change.
    pub skip_reload: bool,
}

#[derive(Debug)]
pub struct Applied {
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub target: PathBuf,
    pub report: Report,
    pub warnings: Vec<Warning>,
}

/// Fold a name the way a menu entry would spell it into a directory name.
///
/// `"Tokyo Night"` and `"<b>Tokyo Night</b>"` both name `tokyo-night`.
pub fn normalize_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;

    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            ' ' | '\t' => out.push('-'),
            _ => out.extend(ch.to_lowercase()),
        }
    }
    out
}

/// Every directory that carries this theme, weakest first.
fn locate(name: &str, theme_dirs: &[PathBuf]) -> Vec<PathBuf> {
    theme_dirs
        .iter()
        .map(|dir| dir.join(name))
        .filter(|candidate| candidate.is_dir())
        .collect()
}

/// Render `request.name` and swap it in as the current theme.
pub fn apply(request: &Request, exec: &dyn Executor) -> Result<Applied, ApplyError> {
    let name = normalize_name(&request.name);
    let sources = locate(&name, &request.theme_dirs);
    if sources.is_empty() {
        return Err(ApplyError::NotFound(name));
    }

    let current = request.state_dir.join(CURRENT_DIR);
    let target = current.join(THEME_DIR);
    // The pid keeps two concurrent applies from sharing a staging directory;
    // the rename at the end is what actually decides which one wins.
    let staging = current.join(format!("next-theme.{}", std::process::id()));

    let outcome = build(request, &sources, &staging);
    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    let (report, warnings) = outcome?;

    swap(&staging, &target)?;
    write_file(&current.join(THEME_NAME_FILE), &format!("{name}\n"))?;

    if !request.skip_reload {
        notify(&target, exec)?;
    }

    Ok(Applied {
        name,
        sources,
        target,
        report,
        warnings,
    })
}

/// Populate a staging directory: theme files first, then whatever the
/// templates add. Copying first is what gives a hand-written file priority.
fn build(
    request: &Request,
    sources: &[PathBuf],
    staging: &Path,
) -> Result<(Report, Vec<Warning>), ApplyError> {
    if staging.exists() {
        std::fs::remove_dir_all(staging).map_err(|e| IoError::Write(staging.into(), e))?;
    }
    std::fs::create_dir_all(staging).map_err(|e| IoError::Write(staging.into(), e))?;

    for source in sources {
        copy_tree(source, staging)?;
    }

    let (palette, warnings) = load_palette(staging)?;
    let report = render_theme(&palette, &request.template_dirs, staging, false)?;

    Ok((report, warnings))
}

/// Replace `target` with `staging`.
///
/// Directories cannot be renamed over a non-empty directory, so the previous
/// theme moves aside first and is removed once the new one is in place.
fn swap(staging: &Path, target: &Path) -> Result<(), IoError> {
    let parent = target
        .parent()
        .ok_or_else(|| IoError::NoParent(target.into()))?;
    std::fs::create_dir_all(parent).map_err(|e| IoError::Write(parent.into(), e))?;

    let previous = target.with_file_name(format!("theme.previous.{}", std::process::id()));
    let had_previous = target.exists();
    if had_previous {
        std::fs::rename(target, &previous).map_err(|e| IoError::Write(target.into(), e))?;
    }

    match std::fs::rename(staging, target) {
        Ok(()) => {
            if had_previous {
                let _ = std::fs::remove_dir_all(&previous);
            }
            Ok(())
        }
        Err(error) => {
            // Put the old theme back rather than leaving the desktop with none.
            if had_previous {
                let _ = std::fs::rename(&previous, target);
            }
            Err(IoError::Write(target.into(), error))
        }
    }
}

/// Hand the new palette to a running desktop.
fn notify(target: &Path, exec: &dyn Executor) -> Result<(), ApplyError> {
    let encode = |name: &str| {
        std::fs::read(target.join(name))
            .ok()
            .map(|bytes| base64(&bytes))
    };
    let colors = encode(PALETTE_FILE);
    let shell = encode(SHELL_FILE);

    notify_omarchy_shell(exec, colors.as_deref(), shell.as_deref())?;
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), IoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| IoError::Write(parent.into(), e))?;
    }
    std::fs::write(path, contents).map_err(|e| IoError::Write(path.into(), e))
}

/// Copy `from` into `to`, overwriting files that already exist.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), IoError> {
    std::fs::create_dir_all(to).map_err(|e| IoError::Write(to.into(), e))?;

    let entries = std::fs::read_dir(from).map_err(|e| IoError::Read(from.into(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| IoError::Read(from.into(), e))?;
        let source = entry.path();
        let destination = to.join(entry.file_name());

        let kind = entry
            .file_type()
            .map_err(|e| IoError::Read(source.clone(), e))?;
        if kind.is_dir() {
            copy_tree(&source, &destination)?;
        } else {
            std::fs::copy(&source, &destination)
                .map_err(|e| IoError::Write(destination.clone(), e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload::RecordingExecutor;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, contents).expect("write");
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        system: PathBuf,
        user: PathBuf,
        templates: PathBuf,
        state: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write(
            &root.join("system/tokyo-night/colors.toml"),
            "background = \"#1a1b26\"\nforeground = \"#a9b1d6\"\n",
        );
        write(
            &root.join("templates/app.conf.tpl"),
            "bg={{ background }}\n",
        );
        Fixture {
            system: root.join("system"),
            user: root.join("user"),
            templates: root.join("templates"),
            state: root.join("state"),
            _dir: dir,
        }
    }

    fn request(f: &Fixture, name: &str) -> Request {
        Request {
            name: name.to_owned(),
            theme_dirs: vec![f.system.clone(), f.user.clone()],
            template_dirs: vec![f.templates.clone()],
            state_dir: f.state.clone(),
            skip_reload: false,
        }
    }

    #[test]
    fn folds_a_display_name_into_a_directory_name() {
        assert_eq!(normalize_name("Tokyo Night"), "tokyo-night");
        assert_eq!(normalize_name("<b>Tokyo Night</b>"), "tokyo-night");
        assert_eq!(normalize_name("ALREADY-fine"), "already-fine");
        assert_eq!(normalize_name("rose-pine"), "rose-pine");
    }

    #[test]
    fn renders_and_swaps_the_theme_in() {
        let f = fixture();
        let recorder = RecordingExecutor::default();

        let applied = apply(&request(&f, "Tokyo Night"), &recorder).expect("apply");

        assert_eq!(applied.name, "tokyo-night");
        assert_eq!(
            std::fs::read_to_string(applied.target.join("app.conf")).expect("read"),
            "bg=#1a1b26\n"
        );
        assert_eq!(
            std::fs::read_to_string(f.state.join("current/theme.name")).expect("read"),
            "tokyo-night\n"
        );
    }

    #[test]
    fn leaves_no_staging_directory_behind() {
        let f = fixture();
        apply(&request(&f, "tokyo-night"), &RecordingExecutor::default()).expect("apply");

        let leftovers: Vec<_> = std::fs::read_dir(f.state.join("current"))
            .expect("readdir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("next-theme") || name.starts_with("theme.previous"))
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }

    #[test]
    fn a_user_theme_overlays_the_shipped_one() {
        let f = fixture();
        // Same name, and it hand-writes the file the template would generate.
        write(&f.user.join("tokyo-night/app.conf"), "hand written\n");

        let applied =
            apply(&request(&f, "tokyo-night"), &RecordingExecutor::default()).expect("apply");

        assert_eq!(applied.sources.len(), 2, "both directories contribute");
        assert_eq!(
            std::fs::read_to_string(applied.target.join("app.conf")).expect("read"),
            "hand written\n"
        );
    }

    #[test]
    fn applying_twice_replaces_the_previous_theme() {
        let f = fixture();
        write(
            &f.system.join("other/colors.toml"),
            "background = \"#ffffff\"\n",
        );

        apply(&request(&f, "tokyo-night"), &RecordingExecutor::default()).expect("first");
        let applied = apply(&request(&f, "other"), &RecordingExecutor::default()).expect("second");

        assert_eq!(
            std::fs::read_to_string(applied.target.join("app.conf")).expect("read"),
            "bg=#ffffff\n"
        );
        assert_eq!(
            std::fs::read_to_string(f.state.join("current/theme.name")).expect("read"),
            "other\n"
        );
    }

    #[test]
    fn hands_the_palette_to_the_running_desktop() {
        let f = fixture();
        let recorder = RecordingExecutor::default();

        apply(&request(&f, "tokyo-night"), &recorder).expect("apply");

        let calls = recorder.calls();
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0].starts_with("omarchy-shell shell applyTheme "),
            "unexpected call: {}",
            calls[0]
        );
        // colors.toml is copied from the theme, so its payload is never empty.
        let payloads: Vec<&str> = calls[0].split(' ').skip(3).collect();
        assert!(
            !payloads[0].is_empty(),
            "colors payload must carry the palette"
        );
    }

    #[test]
    fn skip_reload_touches_nothing_external() {
        let f = fixture();
        let recorder = RecordingExecutor::default();
        let mut req = request(&f, "tokyo-night");
        req.skip_reload = true;

        apply(&req, &recorder).expect("apply");

        assert!(recorder.calls().is_empty());
    }

    #[test]
    fn an_unknown_theme_is_an_error_that_names_it() {
        let f = fixture();
        let error =
            apply(&request(&f, "Nope"), &RecordingExecutor::default()).expect_err("should fail");
        assert!(matches!(error, ApplyError::NotFound(name) if name == "nope"));
    }

    #[test]
    fn a_failed_render_leaves_the_previous_theme_in_place() {
        let f = fixture();
        apply(&request(&f, "tokyo-night"), &RecordingExecutor::default()).expect("first");

        // A theme whose directory exists but carries no palette cannot render.
        std::fs::create_dir_all(f.system.join("broken")).expect("mkdir");
        let failed = apply(&request(&f, "broken"), &RecordingExecutor::default());

        assert!(failed.is_err());
        assert_eq!(
            std::fs::read_to_string(f.state.join("current/theme/app.conf")).expect("read"),
            "bg=#1a1b26\n",
            "the working theme must survive a failed apply"
        );
    }
}
