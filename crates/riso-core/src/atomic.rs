//! Atomic file replacement.
//!
//! Consumers watch these files and reload on change, so a half-written file is
//! a visible glitch. Writing beside the target and renaming makes the swap a
//! single filesystem operation.

use std::path::{Path, PathBuf};

use crate::error::IoError;

pub fn write_atomic(target: &Path, contents: &str) -> Result<(), IoError> {
    write_atomic_bytes(target, contents.as_bytes())
}

pub fn write_atomic_bytes(target: &Path, contents: &[u8]) -> Result<(), IoError> {
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| IoError::NoParent(target.into()))?;
    std::fs::create_dir_all(parent).map_err(|e| IoError::Write(parent.into(), e))?;

    let temporary = temporary_path(target);
    std::fs::write(&temporary, contents).map_err(|e| IoError::Write(temporary.clone(), e))?;

    // Same directory, so the rename cannot cross a filesystem boundary.
    match std::fs::rename(&temporary, target) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(IoError::Write(target.into(), error))
        }
    }
}

/// A sibling path that keeps the full file name.
///
/// Appending rather than replacing the extension keeps `gtk.css` and `gtk.ini`
/// from colliding on the same temporary file.
fn temporary_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "riso".to_owned());
    target.with_file_name(format!(".{name}.riso-tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_new_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("out.css");
        write_atomic(&target, "ciao").expect("write");
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "ciao");
    }

    #[test]
    fn replaces_an_existing_file_leaving_nothing_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("out.css");
        std::fs::write(&target, "old").expect("seed");
        write_atomic(&target, "new").expect("write");

        assert_eq!(std::fs::read_to_string(&target).expect("read"), "new");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("readdir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name() != std::ffi::OsStr::new("out.css"))
            .collect();
        assert!(leftovers.is_empty(), "no temporary file may survive");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("a/b/out.css");
        write_atomic(&target, "x").expect("write");
        assert!(target.exists());
    }

    #[test]
    fn keeps_siblings_that_differ_only_by_extension_apart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let css = dir.path().join("gtk.css");
        let ini = dir.path().join("gtk.ini");
        assert_ne!(temporary_path(&css), temporary_path(&ini));

        write_atomic(&css, "css").expect("write css");
        write_atomic(&ini, "ini").expect("write ini");
        assert_eq!(std::fs::read_to_string(&css).expect("read"), "css");
        assert_eq!(std::fs::read_to_string(&ini).expect("read"), "ini");
    }
}
