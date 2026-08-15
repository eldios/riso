//! Applying a theme: from a name to the files on disk, and a live desktop.
//!
//! The step order is load-bearing. Rendering happens in a staging directory
//! and only a finished staging directory is swapped in, so a reader never sees
//! a half-written theme, and a failed render leaves the previous one in place.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::background;
use crate::desktop::{Desktop, Payload};
use crate::error::IoError;
use crate::palette::Warning;
use crate::plugin;
use crate::reload::{Executor, ReloadError};
use crate::theme::{load_palette, render_theme, Options as RenderOptions, Report};

const PALETTE_FILE: &str = "colors.toml";
const SHELL_FILE: &str = "shell.toml";
const CURRENT_DIR: &str = "current";
const THEME_DIR: &str = "theme";
const THEME_NAME_FILE: &str = "theme.name";
const BACKGROUNDS_DIR: &str = "backgrounds";
const BACKGROUND_LINK: &str = "background";

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("no theme named '{0}' in any theme directory")]
    NotFound(String),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Reload(#[from] ReloadError),
    #[error(transparent)]
    Plugin(crate::plugin::PluginError),
}

/// Which parts of a theme an apply touches.
///
/// A theme is not one thing: config files, a wallpaper, and the retinting of
/// applications that read neither. Applying them separately is what lets a
/// wallpaper change without disturbing a hand-tuned palette.
#[derive(Debug, Clone)]
pub struct Parts {
    /// Render the theme's config files.
    pub files: bool,
    /// Pick and link the wallpaper.
    pub background: bool,
    /// Run the per-application retint hooks.
    pub hooks: bool,
    /// Restrict rendering to these output names, e.g. `ghostty.conf`.
    /// Empty means every template. Naming any of them keeps the files already
    /// in place, so a partial apply adds to the current theme instead of
    /// replacing it.
    pub only: Vec<String>,
}

impl Default for Parts {
    fn default() -> Self {
        Self {
            files: true,
            background: true,
            hooks: true,
            only: Vec::new(),
        }
    }
}

impl Parts {
    /// True when the apply is narrower than the whole theme, and so must build
    /// on what is already there rather than start from nothing.
    fn is_partial(&self) -> bool {
        !self.only.is_empty()
    }
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
    /// Extra directories to look in for this theme's wallpapers.
    pub background_dirs: Vec<PathBuf>,
    /// Commands run after the swap to retint applications that read neither
    /// the palette nor the generated files.
    pub hooks: Vec<String>,
    /// Which parts of the theme to apply.
    pub parts: Parts,
    /// Fall back to the templates compiled into the binary for outputs that
    /// no template directory claims.
    pub builtin_templates: bool,
    /// Plugin directories, weakest first.
    pub plugin_dirs: Vec<PathBuf>,
    /// Home directory, for expanding the `~/` in plugin targets.
    pub home: PathBuf,
    /// Which desktop to notify once the theme is in place.
    pub desktop: Desktop,
    /// Skip telling the running desktop about the change.
    pub skip_reload: bool,
}

#[derive(Debug, Default)]
pub struct Applied {
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub target: PathBuf,
    pub report: Report,
    pub warnings: Vec<Warning>,
    /// The wallpaper now in use, when one was chosen.
    pub background: Option<PathBuf>,
    /// Hooks that were run, in the order they were run.
    pub hooks_run: Vec<String>,
    /// What each plugin did.
    pub plugins: Vec<plugin::Applied>,
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

/// Apply `request.name`, in whole or in the parts the request names.
pub fn apply(request: &Request, exec: &dyn Executor) -> Result<Applied, ApplyError> {
    let name = normalize_name(&request.name);
    let sources = locate(&name, &request.theme_dirs);
    if sources.is_empty() {
        return Err(ApplyError::NotFound(name));
    }

    let current = request.state_dir.join(CURRENT_DIR);
    let target = current.join(THEME_DIR);

    let mut applied = Applied {
        name: name.clone(),
        sources: sources.clone(),
        target: target.clone(),
        ..Default::default()
    };

    if request.parts.files {
        // The pid keeps two concurrent applies from sharing a staging
        // directory; the rename at the end decides which one wins.
        let staging = current.join(format!("next-theme.{}", std::process::id()));

        let outcome = build(request, &sources, &staging, &target);
        if outcome.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        let (report, warnings) = outcome?;
        applied.report = report;
        applied.warnings = warnings;

        swap(&staging, &target)?;
        write_file(&current.join(THEME_NAME_FILE), &format!("{name}\n"))?;
    }

    if request.parts.background {
        applied.background = set_background(request, &name, &target, &current)?;
    }

    if !request.skip_reload {
        notify(request.desktop, &target, exec)?;
    }

    if request.parts.hooks {
        applied.hooks_run = run_hooks(exec, &request.hooks);
        applied.plugins = run_plugins(request, &target, exec)?;
    }

    Ok(applied)
}

/// Populate a staging directory: theme files first, then whatever the
/// templates add. Copying first is what gives a hand-written file priority.
///
/// A partial apply seeds the staging directory from the theme already in
/// place, so files it does not name survive.
fn build(
    request: &Request,
    sources: &[PathBuf],
    staging: &Path,
    target: &Path,
) -> Result<(Report, Vec<Warning>), ApplyError> {
    if staging.exists() {
        std::fs::remove_dir_all(staging).map_err(|e| IoError::Write(staging.into(), e))?;
    }
    std::fs::create_dir_all(staging).map_err(|e| IoError::Write(staging.into(), e))?;

    if request.parts.is_partial() && target.is_dir() {
        copy_tree(target, staging)?;
    }

    for source in sources {
        copy_tree(source, staging)?;
    }

    let (palette, warnings) = load_palette(staging)?;
    let report = render_theme(
        &palette,
        staging,
        &RenderOptions {
            template_dirs: request.template_dirs.clone(),
            only: request.parts.only.clone(),
            dry_run: false,
            builtin: request.builtin_templates,
        },
    )?;

    Ok((report, warnings))
}

/// Choose the theme's next wallpaper and point the current-background link at
/// it. A theme with no images leaves the link untouched.
fn set_background(
    request: &Request,
    name: &str,
    target: &Path,
    current: &Path,
) -> Result<Option<PathBuf>, ApplyError> {
    // User directories are named after the theme; the theme's own images sit
    // beside its config files.
    let mut dirs: Vec<PathBuf> = request
        .background_dirs
        .iter()
        .map(|dir| dir.join(name))
        .collect();
    dirs.push(target.join(BACKGROUNDS_DIR));

    let images = background::candidates(&dirs);
    let link_path = current.join(BACKGROUND_LINK);
    let showing = background::current(&link_path);

    let Some(chosen) = background::next(&images, showing.as_deref()) else {
        return Ok(None);
    };
    background::link(&link_path, &chosen)?;
    Ok(Some(chosen))
}

/// Render every plugin against the theme that was just applied.
///
/// Plugins reach applications that read neither the palette nor the generated
/// files, so they run after the swap and read the palette from what landed.
fn run_plugins(
    request: &Request,
    target: &Path,
    exec: &dyn Executor,
) -> Result<Vec<plugin::Applied>, ApplyError> {
    if request.plugin_dirs.is_empty() {
        return Ok(Vec::new());
    }
    let plugins = plugin::discover(&request.plugin_dirs).map_err(ApplyError::Plugin)?;
    if plugins.is_empty() {
        return Ok(Vec::new());
    }

    let (palette, _) = load_palette(target)?;
    let mut store = crate::snapshot::Store::open(&request.state_dir.join("ownership"))?;

    plugins
        .iter()
        .map(|p| {
            plugin::apply(p, &palette, &request.home, Some(&mut store), exec)
                .map_err(ApplyError::Plugin)
        })
        .collect()
}

/// Run the retint hooks for applications that read neither the palette nor the
/// generated files.
///
/// A hook that cannot start is skipped rather than fatal: the theme is already
/// on disk, and losing it because one editor is absent would be worse.
fn run_hooks(exec: &dyn Executor, hooks: &[String]) -> Vec<String> {
    hooks
        .iter()
        .filter(|hook| !hook.trim().is_empty())
        .filter_map(|hook| {
            let mut words = hook.split_whitespace();
            let program = words.next()?;
            let args: Vec<String> = words.map(str::to_owned).collect();
            exec.run(program, &args).ok().map(|()| hook.clone())
        })
        .collect()
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

/// Hand the new theme to the running desktop, in whatever form it wants it.
fn notify(desktop: Desktop, target: &Path, exec: &dyn Executor) -> Result<(), ApplyError> {
    let read = |name: &str| std::fs::read(target.join(name)).ok();
    let payload = Payload {
        colors: read(PALETTE_FILE),
        shell: read(SHELL_FILE),
    };

    desktop.reload(exec, &payload)?;
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
            background_dirs: Vec::new(),
            hooks: Vec::new(),
            parts: Parts::default(),
            // The fixture asserts on its own templates, so the built-ins would
            // only add noise.
            builtin_templates: false,
            plugin_dirs: Vec::new(),
            home: f.state.clone(),
            desktop: Desktop::Omarchy,
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
