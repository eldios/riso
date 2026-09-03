//! Theme names as people and menus spell them, mapped onto the
//! directories that carry them.

use std::path::PathBuf;

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
pub(crate) fn locate(name: &str, theme_dirs: &[PathBuf]) -> Vec<PathBuf> {
    theme_dirs
        .iter()
        .map(|dir| dir.join(name))
        .filter(|candidate| candidate.is_dir())
        .collect()
}

/// A name reduced to what no spelling can disagree on: its letters and
/// digits, lowercased.
fn loose(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The directory name `folded` denotes, when the folded spelling itself is
/// not on disk: the one theme directory whose loose form matches. Menus and
/// hands spell names in ways folding alone cannot predict ("CyberPunkRED"
/// for a `cyberpunk-red` directory), so the comparison loosens both sides.
/// More than one distinct match is nobody's guess to make: none is returned.
pub(crate) fn resolve_name(folded: &str, theme_dirs: &[PathBuf]) -> Option<String> {
    let wanted = loose(folded);
    let mut found: Option<String> = None;

    for dir in theme_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if loose(&name) == wanted {
                match &found {
                    Some(existing) if *existing != name => return None,
                    _ => found = Some(name),
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camelcase_spelling_finds_the_dashed_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("cyberpunk-red")).expect("mkdir");
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(
            resolve_name(&normalize_name("CyberPunkRED"), &dirs).as_deref(),
            Some("cyberpunk-red")
        );
        assert_eq!(
            resolve_name(&normalize_name("Cyberpunk Red"), &dirs).as_deref(),
            Some("cyberpunk-red")
        );
    }

    #[test]
    fn two_directories_matching_loosely_resolve_to_neither() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("tokyo-night")).expect("mkdir");
        std::fs::create_dir(dir.path().join("tokyonight")).expect("mkdir");
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(resolve_name("tokyo.night", &dirs), None);
    }

    #[test]
    fn folds_a_display_name_into_a_directory_name() {
        assert_eq!(normalize_name("Tokyo Night"), "tokyo-night");
        assert_eq!(normalize_name("<b>Tokyo Night</b>"), "tokyo-night");
        assert_eq!(normalize_name("ALREADY-fine"), "already-fine");
        assert_eq!(normalize_name("rose-pine"), "rose-pine");
    }
}
