//! Per-section overrides of the generated `shell.toml`.
//!
//! A theme that only wants to restyle the lock screen ships `shell.lock.toml`
//! instead of a whole `shell.toml`. The named section is replaced wholesale,
//! not merged key by key: the override states the final contents of that
//! section.

use std::path::{Path, PathBuf};

use crate::atomic::write_atomic;
use crate::error::IoError;

const SHELL_FILE: &str = "shell.toml";

/// Replace `section` in `document` with `body`.
///
/// A section that is not present is appended, so an override always lands.
pub fn apply_section(document: &str, section: &str, body: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    let mut emitted = false;
    let mut lines = 0;

    let replacement = |out: &mut Vec<String>| {
        out.push(format!("[{section}]"));
        out.extend(strip_header(body, section).lines().map(str::to_owned));
    };

    for line in document.lines() {
        lines += 1;
        if is_section_header(line) {
            if inside {
                if !emitted {
                    replacement(&mut out);
                    emitted = true;
                }
                inside = false;
            }
            if names_section(line, section) {
                inside = true;
                continue;
            }
        }
        if !inside {
            out.push(line.to_owned());
        }
    }

    if inside || !emitted {
        if lines > 0 {
            out.push(String::new());
        }
        replacement(&mut out);
    }

    let mut rendered = out.join("\n");
    rendered.push('\n');
    rendered
}

/// Apply every `shell.<section>.toml` sitting in `dir` to its `shell.toml`.
///
/// Returns the overrides that were applied, in the order they were applied.
pub fn apply_overrides(dir: &Path) -> Result<Vec<PathBuf>, IoError> {
    let shell = dir.join(SHELL_FILE);
    if !shell.exists() {
        return Ok(Vec::new());
    }

    let mut applied = Vec::new();
    for path in overrides_in(dir)? {
        let Some(section) = section_of(&path) else {
            continue;
        };
        let body = std::fs::read_to_string(&path).map_err(|e| IoError::Read(path.clone(), e))?;
        let document =
            std::fs::read_to_string(&shell).map_err(|e| IoError::Read(shell.clone(), e))?;

        write_atomic(&shell, &apply_section(&document, &section, &body))?;
        applied.push(path);
    }

    Ok(applied)
}

fn overrides_in(dir: &Path) -> Result<Vec<PathBuf>, IoError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(IoError::Read(dir.into(), e)),
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with("shell.") && name.ends_with(".toml") && name != SHELL_FILE
                })
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// `shell.lock.toml` targets the `lock` section.
fn section_of(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    let section = name.strip_prefix("shell.")?.strip_suffix(".toml")?;
    if section.is_empty()
        || !section
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(section.to_owned())
}

/// A line that is nothing but a `[name]` header, possibly with a comment.
fn is_section_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let Some(close) = rest.find(']') else {
        return false;
    };
    if close == 0 {
        return false;
    }
    let after = rest[close + 1..].trim_start();
    after.is_empty() || after.starts_with('#')
}

fn names_section(line: &str, section: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_prefix(section))
        .and_then(|rest| rest.strip_prefix(']'))
        .map(|after| {
            let after = after.trim_start();
            after.is_empty() || after.starts_with('#')
        })
        .unwrap_or(false)
}

/// Drop the override's own `[section]` header, which is optional in the file.
fn strip_header(body: &str, section: &str) -> String {
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !(is_section_header(line) && names_section(line, section)))
        .collect();
    kept.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A blank line trailing the replaced section belongs to that section and
    // goes with it. Every expectation here was taken from the upstream awk.
    #[test]
    fn replaces_a_section_in_the_middle() {
        let document = "[bar]\na = 1\n\n[lock]\nold = 1\nstale = 2\n\n[menu]\nb = 2\n";
        let got = apply_section(document, "lock", "text = \"#fff\"\n");
        assert_eq!(got, "[bar]\na = 1\n\n[lock]\ntext = \"#fff\"\n[menu]\nb = 2\n");
    }

    #[test]
    fn drops_the_overrides_own_header() {
        let document = "[lock]\nold = 1\n\n[menu]\nb = 2\n";
        let got = apply_section(document, "lock", "[lock]\ntext = \"#fff\"\n");
        assert_eq!(got, "[lock]\ntext = \"#fff\"\n[menu]\nb = 2\n");
    }

    #[test]
    fn appends_a_section_the_document_lacks() {
        let got = apply_section("[bar]\na = 1\n", "lock", "text = \"#fff\"\n");
        assert_eq!(got, "[bar]\na = 1\n\n[lock]\ntext = \"#fff\"\n");
    }

    #[test]
    fn replaces_a_trailing_section() {
        let got = apply_section("[bar]\na = 1\n\n[lock]\nold = 1\n", "lock", "text = \"#fff\"\n");
        assert_eq!(got, "[bar]\na = 1\n\n\n[lock]\ntext = \"#fff\"\n");
    }

    #[test]
    fn keeps_a_commented_header_recognizable() {
        let document = "[lock] # the lock screen\nold = 1\n[menu]\nb = 2\n";
        let got = apply_section(document, "lock", "text = \"#fff\"\n");
        assert_eq!(got, "[lock]\ntext = \"#fff\"\n[menu]\nb = 2\n");
    }

    #[test]
    fn leaves_a_key_that_merely_looks_like_a_header_alone() {
        let document = "[bar]\nkeys = [1, 2]\n";
        let got = apply_section(document, "lock", "x = 1\n");
        assert_eq!(got, "[bar]\nkeys = [1, 2]\n\n[lock]\nx = 1\n");
    }

    #[test]
    fn derives_the_section_from_the_file_name() {
        assert_eq!(
            section_of(Path::new("/t/shell.lock.toml")).as_deref(),
            Some("lock")
        );
        assert_eq!(
            section_of(Path::new("/t/shell.image-picker.toml")).as_deref(),
            Some("image-picker")
        );
        assert_eq!(section_of(Path::new("/t/shell.toml")), None);
        assert_eq!(section_of(Path::new("/t/shell...toml")), None);
    }

    #[test]
    fn applies_an_override_file_to_the_shell_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("shell.toml"), "[lock]\nold = 1\n[menu]\nb = 2\n")
            .expect("seed");
        std::fs::write(dir.path().join("shell.lock.toml"), "text = \"#fff\"\n").expect("seed");

        let applied = apply_overrides(dir.path()).expect("apply");

        assert_eq!(applied.len(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("shell.toml")).expect("read"),
            "[lock]\ntext = \"#fff\"\n[menu]\nb = 2\n"
        );
    }

    #[test]
    fn does_nothing_without_a_shell_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("shell.lock.toml"), "text = \"#fff\"\n").expect("seed");
        assert!(apply_overrides(dir.path()).expect("apply").is_empty());
    }
}
