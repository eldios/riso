//! Checking that a theme is data and nothing else.
//!
//! Themes in circulation ship shell scripts and bind them to keys, and the
//! config formats they use can name a program to run. A catalog that installs
//! such a theme with one command is a way to distribute software, so this is
//! the gate that keeps a theme to being a palette and some files.
//!
//! It is meant to run in a catalog's CI, and it runs on the client at install
//! time. Trusting the server is not an option: installing straight from a git
//! URL never passed through any catalog at all.

use std::path::{Path, PathBuf};

use crate::error::IoError;

/// Directives that name something to run or something to pull in.
///
/// The first one appears as a *field* in Hyprland's
/// `bindd = KEY, description, <that word>, command`, not only as a key. That
/// is why whole-token matching anywhere on the line is the only thing that
/// catches it: a check for lines starting with it walks straight past a theme
/// that binds a script to a key.
const DANGEROUS_WORDS: &[&str] = &[
    "exec",
    "exec-once",
    "exec-shutdown",
    "execr",
    "source",
    "include",
    "plugin",
];

/// Directories that belong to the repository rather than to the theme.
///
/// A clone carries git's sample hooks, which are executable and full of the
/// very directives this looks for. Scanning them would refuse every theme
/// installed from git, which is all of them.
const REPOSITORY_DIRS: &[&str] = &[".git", ".github", ".hg", ".svn"];

/// Extensions whose contents are not text worth scanning.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "ttf", "otf", "woff", "woff2", "zip", "gz",
    "xz", "zst",
];

/// Limits a catalog places on what it will carry.
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub require_license: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 5 * 1024 * 1024,
            max_total_bytes: 20 * 1024 * 1024,
            require_license: true,
        }
    }
}

/// Something about a theme that a catalog should not accept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A file the filesystem would let you run.
    Runnable(PathBuf),
    /// A directive that names a program to run or a file to pull in.
    Directive {
        path: PathBuf,
        line: usize,
        word: String,
        text: String,
    },
    /// A path that leaves the theme directory.
    EscapingPath {
        path: PathBuf,
        line: usize,
        text: String,
    },
    /// A symlink, which can point anywhere at all.
    Symlink(PathBuf),
    NoPalette,
    NoLicense,
    FileTooLarge {
        path: PathBuf,
        bytes: u64,
    },
    ThemeTooLarge {
        bytes: u64,
    },
}

impl Finding {
    /// Whether this alone should stop an install.
    ///
    /// Size and licensing are catalog policy; the rest is the theme trying to
    /// be a program.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::Runnable(_)
                | Self::Directive { .. }
                | Self::EscapingPath { .. }
                | Self::Symlink(_)
                | Self::NoPalette
        )
    }

    pub fn describe(&self) -> String {
        match self {
            Self::Runnable(path) => format!("{}: has the executable bit set", path.display()),
            Self::Directive {
                path,
                line,
                word,
                text,
            } => format!("{}:{line}: '{word}' directive: {text}", path.display()),
            Self::EscapingPath { path, line, text } => {
                format!("{}:{line}: path leaves the theme: {text}", path.display())
            }
            Self::Symlink(path) => format!("{}: is a symlink", path.display()),
            Self::NoPalette => "no colors.toml: this is not a theme".to_owned(),
            Self::NoLicense => "no LICENSE file".to_owned(),
            Self::FileTooLarge { path, bytes } => {
                format!(
                    "{}: {bytes} bytes is over the per-file limit",
                    path.display()
                )
            }
            Self::ThemeTooLarge { bytes } => format!("{bytes} bytes is over the theme limit"),
        }
    }
}

/// Inspect a theme directory.
pub fn validate(theme_dir: &Path, limits: &Limits) -> Result<Vec<Finding>, IoError> {
    let mut findings = Vec::new();

    if !theme_dir.join("colors.toml").is_file() {
        findings.push(Finding::NoPalette);
    }
    if limits.require_license && !has_license(theme_dir) {
        findings.push(Finding::NoLicense);
    }

    let mut total = 0u64;
    walk(theme_dir, theme_dir, limits, &mut total, &mut findings)?;

    if total > limits.max_total_bytes {
        findings.push(Finding::ThemeTooLarge { bytes: total });
    }
    Ok(findings)
}

fn has_license(dir: &Path) -> bool {
    ["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"]
        .iter()
        .any(|name| dir.join(name).is_file())
}

fn walk(
    root: &Path,
    dir: &Path,
    limits: &Limits,
    total: &mut u64,
    findings: &mut Vec<Finding>,
) -> Result<(), IoError> {
    let entries = std::fs::read_dir(dir).map_err(|e| IoError::Read(dir.into(), e))?;
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| IoError::Read(path.clone(), e))?;

        if metadata.file_type().is_symlink() {
            findings.push(Finding::Symlink(relative(root, &path)));
            continue;
        }
        if metadata.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if REPOSITORY_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk(root, &path, limits, total, findings)?;
            continue;
        }

        *total += metadata.len();
        if metadata.len() > limits.max_file_bytes {
            findings.push(Finding::FileTooLarge {
                path: relative(root, &path),
                bytes: metadata.len(),
            });
        }
        if is_runnable(&metadata) {
            findings.push(Finding::Runnable(relative(root, &path)));
        }
        if !is_binary(&path) {
            scan(root, &path, findings)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn is_runnable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_runnable(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn is_binary(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| BINARY_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Read a file line by line looking for directives and escaping paths.
fn scan(root: &Path, path: &Path, findings: &mut Vec<Finding>) -> Result<(), IoError> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        // Not valid UTF-8, so not a config file worth scanning.
        return Ok(());
    };

    for (index, line) in contents.lines().enumerate() {
        let number = index + 1;
        let code = line.split('#').next().unwrap_or(line);

        if let Some(word) = dangerous_word(code) {
            findings.push(Finding::Directive {
                path: relative(root, path),
                line: number,
                word,
                text: line.trim().to_owned(),
            });
        }
        if escapes(code) {
            findings.push(Finding::EscapingPath {
                path: relative(root, path),
                line: number,
                text: line.trim().to_owned(),
            });
        }
    }
    Ok(())
}

/// The first dangerous word appearing as a whole token on the line.
fn dangerous_word(line: &str) -> Option<String> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .find(|token| {
            let lowered = token.to_ascii_lowercase();
            DANGEROUS_WORDS.contains(&lowered.as_str())
        })
        .map(|token| token.to_ascii_lowercase())
}

/// A path that climbs out of the theme, or reaches into the home directory.
fn escapes(line: &str) -> bool {
    line.contains("../") || line.contains("$HOME") || line.contains("~/")
}

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(dir: &Path) -> PathBuf {
        let path = dir.join("theme");
        std::fs::create_dir_all(&path).expect("mkdir");
        std::fs::write(path.join("colors.toml"), "background = \"#000000\"\n").expect("write");
        std::fs::write(path.join("LICENSE"), "MIT\n").expect("write");
        path
    }

    fn findings(path: &Path) -> Vec<Finding> {
        validate(path, &Limits::default()).expect("validate")
    }

    #[test]
    fn a_plain_theme_passes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::fs::write(path.join("ghostty.conf"), "background = #000000\n").expect("write");

        assert_eq!(findings(&path), vec![]);
    }

    #[test]
    fn catches_a_runner_hidden_as_a_field_in_a_binding() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        // The real shape from a shipped theme: the dangerous word is the
        // fourth field, so a check for lines starting with it finds nothing.
        std::fs::write(
            path.join("hyprland.conf"),
            "bindd = SUPER+ALT, BACKSPACE, Toggle dimming, exec, /home/x/dim.sh\n",
        )
        .expect("write");

        let found = findings(&path);
        assert!(
            found
                .iter()
                .any(|f| matches!(f, Finding::Directive { word, .. } if word == "exec")),
            "got {found:?}"
        );
        assert!(found.iter().any(Finding::is_fatal));
    }

    #[test]
    fn catches_the_obvious_directives_too() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::fs::write(
            path.join("hyprland.conf"),
            "exec-once = /usr/bin/thing\nsource = other.conf\n",
        )
        .expect("write");

        let words: Vec<String> = findings(&path)
            .into_iter()
            .filter_map(|f| match f {
                Finding::Directive { word, .. } => Some(word),
                _ => None,
            })
            .collect();
        assert!(words.contains(&"exec-once".to_owned()));
        assert!(words.contains(&"source".to_owned()));
    }

    #[test]
    fn ignores_a_directive_that_is_only_a_comment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::fs::write(path.join("notes.conf"), "# exec = nothing here\n").expect("write");

        assert_eq!(findings(&path), vec![]);
    }

    #[test]
    fn does_not_flag_a_longer_word_that_merely_contains_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::fs::write(path.join("app.conf"), "executive_summary = blue\n").expect("write");

        assert_eq!(findings(&path), vec![], "whole tokens only");
    }

    #[test]
    fn catches_a_file_with_the_executable_bit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        let script = path.join("dimming.sh");
        std::fs::write(&script, "#!/bin/sh\necho hi\n").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }

        assert!(findings(&path)
            .iter()
            .any(|f| matches!(f, Finding::Runnable(_))));
    }

    #[test]
    fn catches_a_path_that_leaves_the_theme() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::fs::write(path.join("app.conf"), "background = ../../etc/passwd\n").expect("write");

        assert!(findings(&path)
            .iter()
            .any(|f| matches!(f, Finding::EscapingPath { .. })));
    }

    #[test]
    fn catches_a_symlink_without_following_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::os::unix::fs::symlink("/etc/passwd", path.join("sneaky.conf")).expect("symlink");

        assert!(findings(&path)
            .iter()
            .any(|f| matches!(f, Finding::Symlink(_))));
    }

    #[test]
    fn reports_a_missing_palette_and_licence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bare");
        std::fs::create_dir_all(&path).expect("mkdir");

        let found = findings(&path);
        assert!(found.contains(&Finding::NoPalette));
        assert!(found.contains(&Finding::NoLicense));
        assert!(Finding::NoPalette.is_fatal());
        assert!(
            !Finding::NoLicense.is_fatal(),
            "licensing is policy, not danger"
        );
    }

    #[test]
    fn reports_a_file_over_the_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        std::fs::write(path.join("huge.png"), vec![0u8; 2048]).expect("write");

        let limits = Limits {
            max_file_bytes: 1024,
            ..Default::default()
        };
        let found = validate(&path, &limits).expect("validate");
        assert!(found
            .iter()
            .any(|f| matches!(f, Finding::FileTooLarge { .. })));
    }

    #[test]
    fn ignores_the_repository_metadata_a_clone_brings_along() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        // What `git clone` leaves behind: executable hooks full of directives.
        let hooks = path.join(".git/hooks");
        std::fs::create_dir_all(&hooks).expect("mkdir");
        let hook = hooks.join("pre-commit.sample");
        std::fs::write(&hook, "#!/bin/sh\nexec git diff-index --check\n").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        assert_eq!(findings(&path), vec![], "a cloned theme must still pass");
    }

    #[test]
    fn does_not_scan_inside_images() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = theme(dir.path());
        // Bytes that would look like a directive if read as text.
        std::fs::write(path.join("preview.png"), b"exec = /bin/sh\n").expect("write");

        assert_eq!(findings(&path), vec![]);
    }
}
