//! Finding themes, and installing them from wherever they live.
//!
//! A theme is an artifact in its own right: the same directory works whether
//! it came from riso's catalog, from Omarchy's, from any git repository, or
//! from `theme init`. Installing is therefore a fetch, never a conversion.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::error::IoError;
use crate::reload::{Executor, ReloadError};

const PALETTE_FILE: &str = "colors.toml";

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog index is not valid JSON: {0}")]
    Index(#[from] serde_json::Error),
    #[error("no theme named '{0}' in the catalog")]
    Unknown(String),
    #[error("'{0}' is already installed; remove it first")]
    Installed(String),
    #[error("nothing named '{0}' is installed")]
    NotInstalled(String),
    #[error("'{0}' does not look like a theme: no colors.toml")]
    NotATheme(PathBuf),
    #[error("'{0}' was installed by a package manager and is read-only to riso")]
    ReadOnly(PathBuf),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Fetch(#[from] ReloadError),
}

/// One theme as the catalog index describes it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// Git repository to clone.
    pub repo: String,
    #[serde(default)]
    pub description: String,
    /// Commit or tag to pin, when the catalog states one.
    #[serde(default)]
    pub rev: Option<String>,
    /// Reason the theme was withdrawn, if it was.
    #[serde(default)]
    pub yanked: Option<String>,
    /// Image to show while browsing, when the catalog names one.
    #[serde(default)]
    pub preview: Option<String>,
}

/// A catalog index: a flat list of themes, published as static JSON.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Index {
    #[serde(default)]
    pub themes: Vec<Entry>,
}

impl Index {
    pub fn parse(json: &str) -> Result<Self, CatalogError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.themes.iter().find(|entry| entry.name == name)
    }
}

/// Download a catalog index.
///
/// Fetching goes through curl for the same reason cloning goes through git:
/// TLS, proxies and redirects are solved problems that belong to the system,
/// not to a ricing tool.
pub fn fetch_index(exec: &dyn Executor, url: &str) -> Result<Index, CatalogError> {
    let json = exec.capture(
        "curl",
        &[
            "--fail".to_owned(),
            "--silent".to_owned(),
            "--show-error".to_owned(),
            "--location".to_owned(),
            url.to_owned(),
        ],
    )?;
    Index::parse(&json)
}

/// A theme found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub name: String,
    pub path: PathBuf,
    /// False for the directories a package manager owns, which are read-only
    /// to us: knowing this is what lets `remove` refuse instead of failing.
    pub removable: bool,
}

/// A directory named by an environment variable, ignoring an empty value.
fn from_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Where a user's own themes live, and where `theme install` puts them.
pub fn user_theme_dir() -> Option<PathBuf> {
    from_env("XDG_CONFIG_HOME")
        .or_else(|| from_env("HOME").map(|home| home.join(".config")))
        .map(|base| base.join("riso/themes"))
}

/// Where riso looks for themes when no directory is given, least specific
/// first.
///
/// `locate` overlays later directories over earlier ones, so this order puts
/// the user's own themes last, where they win over what a package or the
/// session provides. `RISO_THEMES` is how a distribution or a session points
/// riso at themes it ships, without every command having to name them.
pub fn default_theme_dirs() -> Vec<PathBuf> {
    let omarchy = crate::config::Config::load_or_default().omarchy_themes;
    default_theme_dirs_with(
        if omarchy {
            from_env("OMARCHY_PATH")
        } else {
            None
        },
        omarchy,
    )
}

/// The search path, given whether an Omarchy install is present and whether
/// its themes are wanted at all.
///
/// On an Omarchy system riso is a drop-in: the themes that desktop ships,
/// and the ones its users install in its own directory, are part of the
/// default view rather than something every command must be told about.
/// Setting `omarchy-themes = false` in the configuration opts out of both.
fn default_theme_dirs_with(omarchy: Option<PathBuf>, omarchy_user: bool) -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/riso/themes"),
        PathBuf::from("/etc/riso/themes"),
    ];

    if let Some(omarchy) = omarchy {
        dirs.push(omarchy.join("themes"));
    }

    if let Some(list) = std::env::var_os("RISO_THEMES") {
        dirs.extend(std::env::split_paths(&list).filter(|p| !p.as_os_str().is_empty()));
    }

    let data = from_env("XDG_DATA_HOME")
        .or_else(|| from_env("HOME").map(|home| home.join(".local/share")))
        .map(|base| base.join("riso/themes"));
    dirs.extend(data);
    if omarchy_user {
        if let Some(config) = from_env("XDG_CONFIG_HOME")
            .or_else(|| from_env("HOME").map(|home| home.join(".config")))
        {
            dirs.push(config.join("omarchy/themes"));
        }
    }
    dirs.extend(user_theme_dir());

    dirs
}

/// Write the theme compiled into riso to `dir`, so that a system with none
/// installed still has one, as a directory the user can read and edit.
///
/// Returns the theme's path. Existing files are left alone: this restores a
/// missing fallback, it does not undo edits to it.
pub fn seed_fallback(dir: &Path) -> Result<PathBuf, IoError> {
    let theme = dir.join(crate::builtin::FALLBACK_THEME);

    for (name, contents) in crate::builtin::FALLBACK_THEME_FILES {
        let path = theme.join(name);
        if !path.exists() {
            crate::atomic::write_atomic(&path, contents)?;
        }
    }
    for (name, contents) in crate::builtin::FALLBACK_THEME_ASSETS {
        let path = theme.join(name);
        if !path.exists() {
            crate::atomic::write_atomic_bytes(&path, contents)?;
        }
    }

    Ok(theme)
}

/// Every theme across the given directories, sorted, first occurrence winning.
///
/// `writable` names the directory a user installs into; themes anywhere else
/// belong to whoever put them there.
pub fn installed(theme_dirs: &[PathBuf], writable: Option<&Path>) -> Vec<Installed> {
    let mut found: Vec<Installed> = Vec::new();

    for dir in theme_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.join(PALETTE_FILE).is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if found.iter().any(|other| other.name == name) {
                continue;
            }
            found.push(Installed {
                name,
                removable: writable.is_some_and(|w| path.starts_with(w)),
                path,
            });
        }
    }

    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Clone a theme into `into`, named after `name`.
///
/// Cloning is delegated to git rather than reimplemented: it is the one tool
/// every system already has for this, and it handles the protocols, the
/// authentication and the pinning.
/// `expect` names the file that proves the clone is what was asked for:
/// `colors.toml` for a theme, `manifest.toml` for a plugin.
pub fn install_from_git(
    exec: &dyn Executor,
    repo: &str,
    rev: Option<&str>,
    name: &str,
    into: &Path,
    expect: &str,
) -> Result<PathBuf, CatalogError> {
    let destination = into.join(name);
    if destination.exists() {
        return Err(CatalogError::Installed(name.to_owned()));
    }
    std::fs::create_dir_all(into).map_err(|e| IoError::Write(into.into(), e))?;

    let mut args = vec!["clone".to_owned(), "--quiet".to_owned()];
    match rev {
        // A pinned revision needs full history to check out; without one the
        // tip is enough and a shallow clone is much smaller.
        Some(_) => {}
        None => args.push("--depth=1".to_owned()),
    }
    args.push(repo.to_owned());
    args.push(destination.to_string_lossy().into_owned());

    // capture, not run: a git failure must fail the install. A checkout that
    // failed quietly would leave the tip installed where the catalog pinned a
    // revision, which is the one substitution this function exists to prevent.
    if let Err(error) = exec.capture("git", &args) {
        let _ = std::fs::remove_dir_all(&destination);
        return Err(error.into());
    }

    if let Some(rev) = rev {
        let checkout = exec.capture(
            "git",
            &[
                "-C".to_owned(),
                destination.to_string_lossy().into_owned(),
                "checkout".to_owned(),
                "--quiet".to_owned(),
                rev.to_owned(),
            ],
        );
        if let Err(error) = checkout {
            let _ = std::fs::remove_dir_all(&destination);
            return Err(error.into());
        }
    }

    // A clone missing that file is not what was asked for, and leaving it
    // behind would make it show up in every later listing.
    if !destination.join(expect).is_file() {
        let _ = std::fs::remove_dir_all(&destination);
        return Err(CatalogError::NotATheme(destination));
    }

    Ok(destination)
}

/// What updating one theme came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Updated {
    /// Moved from one revision to another.
    Moved { from: String, to: String },
    /// Already at the target revision.
    Current,
    /// Not a git clone, so there is nothing to pull from.
    NotAClone,
}

/// Bring one installed theme to `rev`, or to its origin's tip without one.
///
/// The clone keeps its history, so the theme's own repository is the source
/// of truth for where updates come from; the catalog only contributes the
/// revision to pin when the theme came from it. The caller validates what
/// arrived and calls back with `rollback` if it must not be kept.
pub fn update_from_git(
    exec: &dyn Executor,
    theme_dir: &Path,
    rev: Option<&str>,
) -> Result<Updated, CatalogError> {
    let dir = theme_dir.to_string_lossy().into_owned();
    let git = |args: &[&str]| {
        let mut full = vec!["-C".to_owned(), dir.clone()];
        full.extend(args.iter().map(|a| (*a).to_owned()));
        exec.capture("git", &full)
    };

    if !theme_dir.join(".git").exists() {
        return Ok(Updated::NotAClone);
    }
    let from = git(&["rev-parse", "HEAD"])?.trim().to_owned();

    match rev {
        Some(rev) => {
            // A pinned revision may be beyond a shallow clone's horizon.
            git(&["fetch", "--quiet", "--unshallow", "origin"])
                .or_else(|_| git(&["fetch", "--quiet", "origin"]))?;
            git(&["checkout", "--quiet", rev])?;
        }
        None => {
            git(&["fetch", "--quiet", "--depth=1", "origin", "HEAD"])?;
            git(&["reset", "--quiet", "--hard", "FETCH_HEAD"])?;
        }
    }

    let to = git(&["rev-parse", "HEAD"])?.trim().to_owned();
    if from == to {
        return Ok(Updated::Current);
    }
    Ok(Updated::Moved { from, to })
}

/// Put a theme back to the revision it was on before an update.
pub fn rollback(exec: &dyn Executor, theme_dir: &Path, rev: &str) -> Result<(), CatalogError> {
    exec.capture(
        "git",
        &[
            "-C".to_owned(),
            theme_dir.to_string_lossy().into_owned(),
            "checkout".to_owned(),
            "--quiet".to_owned(),
            rev.to_owned(),
        ],
    )?;
    Ok(())
}

/// Remove an installed theme, refusing anything riso does not own.
pub fn remove(
    name: &str,
    theme_dirs: &[PathBuf],
    writable: &Path,
) -> Result<PathBuf, CatalogError> {
    let found = installed(theme_dirs, Some(writable));
    let target = found
        .iter()
        .find(|theme| theme.name == name)
        .ok_or_else(|| CatalogError::NotInstalled(name.to_owned()))?;

    if !target.removable {
        return Err(CatalogError::ReadOnly(target.path.clone()));
    }
    std::fs::remove_dir_all(&target.path).map_err(|e| IoError::Write(target.path.clone(), e))?;
    Ok(target.path.clone())
}

/// A name that is safe to use as a directory.
///
/// Theme names reach the filesystem, so a name that walks out of the install
/// directory has to be refused rather than sanitized into something else.
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Guess a theme name from a repository URL.
pub fn name_from_repo(repo: &str) -> String {
    repo.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(repo)
        .trim_end_matches(".git")
        .trim_start_matches("riso-theme-")
        .trim_start_matches("omarchy-theme-")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reload::RecordingExecutor;

    fn theme_at(dir: &Path, name: &str) {
        let path = dir.join(name);
        std::fs::create_dir_all(&path).expect("mkdir");
        std::fs::write(path.join(PALETTE_FILE), "background = \"#000000\"\n").expect("write");
    }

    #[test]
    fn updates_a_clone_and_rolls_it_back() {
        use crate::reload::ProcessExecutor;
        let exec = ProcessExecutor;
        let dir = tempfile::tempdir().expect("tempdir");
        let origin = dir.path().join("origin");
        std::fs::create_dir_all(&origin).expect("mkdir");
        let git = |cwd: &Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .expect("git");
            assert!(out.status.success(), "{args:?}: {:?}", out);
        };
        git(&origin, &["init", "--quiet"]);
        std::fs::write(origin.join("colors.toml"), "background = \"#000\"\n").expect("write");
        git(&origin, &["add", "-A"]);
        git(
            &origin,
            &["commit", "--quiet", "--no-gpg-sign", "-m", "one"],
        );
        let clone = dir.path().join("theme");
        git(
            dir.path(),
            &["clone", "--quiet", origin.to_str().unwrap(), "theme"],
        );
        std::fs::write(origin.join("colors.toml"), "background = \"#fff\"\n").expect("write");
        git(&origin, &["add", "-A"]);
        git(
            &origin,
            &["commit", "--quiet", "--no-gpg-sign", "-m", "two"],
        );

        let updated = update_from_git(&exec, &clone, None).expect("update");
        let Updated::Moved { from, to } = updated else {
            panic!("expected a move, got {updated:?}");
        };
        assert_ne!(from, to);
        assert_eq!(
            update_from_git(&exec, &clone, None).expect("again"),
            Updated::Current
        );

        rollback(&exec, &clone, &from).expect("rollback");
        let head = std::process::Command::new("git")
            .args(["-C", clone.to_str().unwrap(), "rev-parse", "HEAD"])
            .output()
            .expect("git");
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), from);

        let plain = dir.path().join("no-git-theme");
        std::fs::create_dir_all(&plain).expect("mkdir");
        assert_eq!(
            update_from_git(&exec, &plain, None).expect("plain"),
            Updated::NotAClone
        );
    }

    #[test]
    fn seeds_the_fallback_as_a_theme_that_validates() {
        let dir = tempfile::tempdir().expect("tempdir");
        let theme = seed_fallback(dir.path()).expect("seed");

        assert_eq!(theme.file_name().unwrap(), crate::builtin::FALLBACK_THEME);
        assert!(theme.join(PALETTE_FILE).is_file());
        assert!(theme.join("LICENSE").is_file());

        let found = installed(&[dir.path().to_path_buf()], Some(dir.path()));
        assert_eq!(found.len(), 1);
        assert!(found[0].removable);
    }

    #[test]
    fn seeding_again_keeps_what_the_user_edited() {
        let dir = tempfile::tempdir().expect("tempdir");
        let theme = seed_fallback(dir.path()).expect("seed");
        std::fs::write(theme.join(PALETTE_FILE), "background = \"#ffffff\"\n").expect("write");

        seed_fallback(dir.path()).expect("seed again");

        let kept = std::fs::read_to_string(theme.join(PALETTE_FILE)).expect("read");
        assert_eq!(kept, "background = \"#ffffff\"\n");
    }

    #[test]
    fn the_search_path_ends_with_the_directory_the_user_owns() {
        let dirs = default_theme_dirs();

        assert_eq!(dirs.first().unwrap(), Path::new("/usr/share/riso/themes"));
        // Last is what `locate` overlays over everything else.
        assert!(dirs.last().unwrap().ends_with("riso/themes"));
        assert_eq!(Some(dirs.last().unwrap()), user_theme_dir().as_ref());
    }

    #[test]
    fn an_omarchy_install_joins_the_search_path_before_the_user() {
        let dirs = default_theme_dirs_with(Some(PathBuf::from("/usr/share/omarchy")), true);
        let omarchy = dirs
            .iter()
            .position(|d| d == Path::new("/usr/share/omarchy/themes"))
            .expect("omarchy themes on the path");
        let user = dirs.len() - 1;
        assert!(omarchy < user, "the user's own themes still win");
    }

    #[test]
    fn opting_out_drops_every_omarchy_directory() {
        let dirs = default_theme_dirs_with(None, false);
        assert!(
            !dirs.iter().any(|d| d.ends_with("omarchy/themes")),
            "no omarchy directory should remain: {dirs:?}"
        );
    }

    #[test]
    fn parses_an_index() {
        let index = Index::parse(
            r#"{"themes":[
                 {"name":"rose-pine","repo":"https://github.com/x/y.git","description":"soho"},
                 {"name":"gone","repo":"https://github.com/x/z.git","yanked":"dmca"}
               ]}"#,
        )
        .expect("index");

        assert_eq!(index.themes.len(), 2);
        assert_eq!(
            index.find("rose-pine").map(|e| e.repo.as_str()),
            Some("https://github.com/x/y.git")
        );
        assert_eq!(
            index.find("gone").and_then(|e| e.yanked.as_deref()),
            Some("dmca")
        );
        assert!(index.find("absent").is_none());
    }

    #[test]
    fn an_empty_index_is_valid() {
        assert!(Index::parse("{}").expect("index").themes.is_empty());
    }

    #[test]
    fn a_malformed_index_is_an_error() {
        assert!(Index::parse("not json").is_err());
    }

    #[test]
    fn fetches_and_parses_an_index() {
        let recorder = RecordingExecutor::default();
        recorder.will_output(&[r#"{"themes":[{"name":"nord","repo":"https://x/n.git"}]}"#]);

        let index = fetch_index(&recorder, "https://riso.example/index.json").expect("fetch");

        assert_eq!(index.themes.len(), 1);
        assert!(recorder.calls()[0].contains("curl --fail --silent"));
        assert!(recorder.calls()[0].ends_with("https://riso.example/index.json"));
    }

    #[test]
    fn lists_themes_across_directories_without_duplicates() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user");
        let system = dir.path().join("system");
        theme_at(&user, "mine");
        theme_at(&system, "shipped");
        theme_at(&system, "mine"); // shadowed by the user copy
        std::fs::create_dir_all(system.join("not-a-theme")).expect("mkdir");

        let found = installed(&[user.clone(), system], Some(&user));

        let names: Vec<&str> = found.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["mine", "shipped"]);
        assert!(found[0].removable, "the user copy is ours to remove");
        assert!(!found[1].removable, "a shipped theme is not");
    }

    #[test]
    fn clones_a_theme_and_pins_it_when_asked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let recorder = RecordingExecutor::default();

        // The recorder does not actually clone, so seed what git would leave.
        theme_at(dir.path(), "rose-pine");
        let result = install_from_git(
            &recorder,
            "https://github.com/x/y.git",
            Some("v1.2.0"),
            "rose-pine",
            dir.path(),
            PALETTE_FILE,
        );

        assert!(
            matches!(result, Err(CatalogError::Installed(_))),
            "existing themes are never overwritten"
        );

        let empty = tempfile::tempdir().expect("tempdir");
        theme_at(empty.path(), "seeded");
        let recorder = RecordingExecutor::default();
        let _ = install_from_git(
            &recorder,
            "https://x/y.git",
            Some("v1"),
            "seeded2",
            empty.path(),
            PALETTE_FILE,
        );
        let calls = recorder.calls();
        assert!(
            calls[0].starts_with("git clone --quiet https://x/y.git"),
            "got {}",
            calls[0]
        );
        assert!(
            !calls[0].contains("--depth"),
            "a pinned revision needs history"
        );
        assert!(calls[1].contains("checkout --quiet v1"));
    }

    #[test]
    fn clones_shallow_when_nothing_is_pinned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let recorder = RecordingExecutor::default();
        let _ = install_from_git(
            &recorder,
            "https://x/y.git",
            None,
            "shallow",
            dir.path(),
            PALETTE_FILE,
        );

        assert!(recorder.calls()[0].contains("--depth=1"));
    }

    #[test]
    fn a_clone_without_a_palette_is_not_kept() {
        let dir = tempfile::tempdir().expect("tempdir");
        let recorder = RecordingExecutor::default();

        let result = install_from_git(
            &recorder,
            "https://x/y.git",
            None,
            "empty",
            dir.path(),
            PALETTE_FILE,
        );

        assert!(matches!(result, Err(CatalogError::NotATheme(_))));
        assert!(
            !dir.path().join("empty").exists(),
            "the failed clone is cleaned up"
        );
    }

    #[test]
    fn a_failed_checkout_fails_the_install_and_keeps_nothing() {
        /// Clones succeed, checkouts fail: the shape of a bad pinned revision.
        struct BadRevExecutor;
        impl crate::reload::Executor for BadRevExecutor {
            fn run(&self, _: &str, _: &[String]) -> Result<(), crate::reload::ReloadError> {
                Ok(())
            }
            fn capture(
                &self,
                program: &str,
                args: &[String],
            ) -> Result<String, crate::reload::ReloadError> {
                if args.iter().any(|a| a == "checkout") {
                    return Err(crate::reload::ReloadError::Failed(
                        program.to_owned(),
                        "pathspec 'v9.9.9' did not match".to_owned(),
                    ));
                }
                Ok(String::new())
            }
        }

        let empty = tempfile::tempdir().expect("tempdir");
        let result = install_from_git(
            &BadRevExecutor,
            "https://x/y.git",
            Some("v9.9.9"),
            "pinned",
            empty.path(),
            PALETTE_FILE,
        );

        assert!(matches!(result, Err(CatalogError::Fetch(_))));
        assert!(
            !empty.path().join("pinned").exists(),
            "a tip left behind by a failed checkout is a silent substitution"
        );
    }

    #[test]
    fn removes_only_what_riso_installed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user");
        let system = dir.path().join("system");
        theme_at(&user, "mine");
        theme_at(&system, "shipped");
        let dirs = [user.clone(), system];

        remove("mine", &dirs, &user).expect("remove");
        assert!(!user.join("mine").exists());

        assert!(matches!(
            remove("shipped", &dirs, &user),
            Err(CatalogError::ReadOnly(_))
        ));
        assert!(matches!(
            remove("absent", &dirs, &user),
            Err(CatalogError::NotInstalled(_))
        ));
    }

    #[test]
    fn refuses_names_that_would_escape_the_install_directory() {
        assert!(is_safe_name("rose-pine"));
        assert!(is_safe_name("tokyo_night.2"));
        assert!(!is_safe_name("../etc"));
        assert!(!is_safe_name("/absolute"));
        assert!(!is_safe_name(".hidden"));
        assert!(!is_safe_name(""));
        assert!(!is_safe_name(".."));
    }

    #[test]
    fn derives_a_name_from_a_repository_url() {
        assert_eq!(
            name_from_repo("https://github.com/x/rose-pine.git"),
            "rose-pine"
        );
        assert_eq!(
            name_from_repo("https://github.com/x/riso-theme-nord"),
            "nord"
        );
        assert_eq!(
            name_from_repo("git@github.com:x/omarchy-theme-Gruvbox.git"),
            "gruvbox"
        );
        assert_eq!(name_from_repo("https://github.com/x/y/"), "y");
    }
}
