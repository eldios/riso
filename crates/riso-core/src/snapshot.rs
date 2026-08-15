//! Knowing what riso touched, so it can put everything back.
//!
//! Before writing over a file that was already there, its contents are copied
//! aside and recorded. `restore` puts the copy back byte for byte, and a file
//! riso created is removed rather than emptied.
//!
//! This is what makes uninstalling possible, and uninstalling cleanly is what
//! makes trying the tool reasonable in the first place.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::error::IoError;

const REGISTRY_FILE: &str = "ownership.json";
const BACKUP_DIR: &str = "originals";

/// What was at a path before riso first wrote to it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// Copy of the original contents, or `None` when nothing was there.
    pub backup: Option<PathBuf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Registry {
    entries: BTreeMap<PathBuf, Entry>,
}

/// The record of everything riso owns.
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    registry: Registry,
}

/// What a restore did to one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restored {
    /// The original contents were put back.
    Contents(PathBuf),
    /// Nothing was there before, so the file was removed.
    Removed(PathBuf),
}

impl Store {
    /// Open the record kept under `root`, creating it if this is the first run.
    pub fn open(root: &Path) -> Result<Self, IoError> {
        std::fs::create_dir_all(root).map_err(|e| IoError::Write(root.into(), e))?;

        let path = root.join(REGISTRY_FILE);
        let registry = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Registry::default(),
            Err(e) => return Err(IoError::Read(path, e)),
        };

        Ok(Self {
            root: root.to_path_buf(),
            registry,
        })
    }

    /// Record what is at `target` before anything writes over it.
    ///
    /// Idempotent, and deliberately so: the first capture is the one that
    /// holds the state from before riso existed. A later one would record
    /// riso's own output as if it were the user's.
    pub fn capture(&mut self, target: &Path) -> Result<(), IoError> {
        if self.registry.entries.contains_key(target) {
            return Ok(());
        }

        let entry = if target.exists() {
            let backup = self.backup_path(target);
            let dir = backup.parent().expect("backup path has a parent");
            std::fs::create_dir_all(dir).map_err(|e| IoError::Write(dir.into(), e))?;
            std::fs::copy(target, &backup).map_err(|e| IoError::Write(backup.clone(), e))?;
            Entry {
                backup: Some(backup),
            }
        } else {
            Entry { backup: None }
        };

        self.registry.entries.insert(target.to_path_buf(), entry);
        self.persist()
    }

    /// Put `target` back the way it was found.
    pub fn restore(&mut self, target: &Path) -> Result<Option<Restored>, IoError> {
        let Some(entry) = self.registry.entries.get(target).cloned() else {
            return Ok(None);
        };

        let outcome = match entry.backup {
            Some(backup) => {
                std::fs::copy(&backup, target).map_err(|e| IoError::Write(target.into(), e))?;
                let _ = std::fs::remove_file(&backup);
                Restored::Contents(target.to_path_buf())
            }
            None => {
                if target.exists() {
                    std::fs::remove_file(target).map_err(|e| IoError::Write(target.into(), e))?;
                }
                Restored::Removed(target.to_path_buf())
            }
        };

        self.registry.entries.remove(target);
        self.persist()?;
        Ok(Some(outcome))
    }

    /// Put everything back, deepest path first so directories empty out from
    /// the inside.
    pub fn restore_all(&mut self) -> Result<Vec<Restored>, IoError> {
        let mut targets: Vec<PathBuf> = self.registry.entries.keys().cloned().collect();
        targets.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

        let mut done = Vec::new();
        for target in targets {
            if let Some(outcome) = self.restore(&target)? {
                done.push(outcome);
            }
        }
        Ok(done)
    }

    pub fn owns(&self, target: &Path) -> bool {
        self.registry.entries.contains_key(target)
    }

    pub fn targets(&self) -> impl Iterator<Item = &PathBuf> {
        self.registry.entries.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.registry.entries.is_empty()
    }

    fn persist(&self) -> Result<(), IoError> {
        let raw = serde_json::to_string_pretty(&self.registry).unwrap_or_default();
        write_atomic(&self.root.join(REGISTRY_FILE), &raw)
    }

    /// A backup name derived from the full path, so two files with the same
    /// name in different directories cannot overwrite each other's copy.
    fn backup_path(&self, target: &Path) -> PathBuf {
        let flattened: String = target
            .to_string_lossy()
            .chars()
            .map(|c| if c == '/' { '%' } else { c })
            .collect();
        self.root
            .join(BACKUP_DIR)
            .join(flattened.trim_start_matches('%'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, contents).expect("write");
    }

    #[test]
    fn puts_a_preexisting_file_back_byte_for_byte() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("app.conf");
        let original = "# hand written, with comments\nkey = value\n";
        write(&target, original);

        let mut store = Store::open(&dir.path().join("state")).expect("open");
        store.capture(&target).expect("capture");
        std::fs::write(&target, "GENERATED").expect("overwrite");

        let outcome = store.restore(&target).expect("restore");

        assert_eq!(outcome, Some(Restored::Contents(target.clone())));
        assert_eq!(std::fs::read_to_string(&target).expect("read"), original);
    }

    #[test]
    fn removes_a_file_that_was_not_there_before() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("generated.conf");

        let mut store = Store::open(&dir.path().join("state")).expect("open");
        store.capture(&target).expect("capture of an absent file");
        write(&target, "GENERATED");

        let outcome = store.restore(&target).expect("restore");

        assert_eq!(outcome, Some(Restored::Removed(target.clone())));
        assert!(!target.exists());
    }

    #[test]
    fn the_first_capture_is_the_one_that_counts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("app.conf");
        write(&target, "ORIGINAL");

        let mut store = Store::open(&dir.path().join("state")).expect("open");
        store.capture(&target).expect("first");
        std::fs::write(&target, "FIRST GENERATION").expect("write");
        store.capture(&target).expect("second");

        store.restore(&target).expect("restore");

        assert_eq!(std::fs::read_to_string(&target).expect("read"), "ORIGINAL");
    }

    #[test]
    fn survives_being_reopened() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().join("state");
        let target = dir.path().join("app.conf");
        write(&target, "ORIGINAL");

        let mut store = Store::open(&state).expect("open");
        store.capture(&target).expect("capture");
        drop(store);

        let mut reopened = Store::open(&state).expect("reopen");
        assert!(reopened.owns(&target));
        std::fs::write(&target, "GENERATED").expect("write");
        reopened.restore(&target).expect("restore");

        assert_eq!(std::fs::read_to_string(&target).expect("read"), "ORIGINAL");
    }

    #[test]
    fn files_with_the_same_name_keep_separate_copies() {
        let dir = tempfile::tempdir().expect("tempdir");
        let one = dir.path().join("a/app.conf");
        let two = dir.path().join("b/app.conf");
        write(&one, "ONE");
        write(&two, "TWO");

        let mut store = Store::open(&dir.path().join("state")).expect("open");
        store.capture(&one).expect("capture one");
        store.capture(&two).expect("capture two");
        std::fs::write(&one, "X").expect("write");
        std::fs::write(&two, "Y").expect("write");

        store.restore_all().expect("restore all");

        assert_eq!(std::fs::read_to_string(&one).expect("read"), "ONE");
        assert_eq!(std::fs::read_to_string(&two).expect("read"), "TWO");
    }

    #[test]
    fn restore_all_leaves_nothing_owned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let existing = dir.path().join("kept.conf");
        let created = dir.path().join("new.conf");
        write(&existing, "KEEP");

        let mut store = Store::open(&dir.path().join("state")).expect("open");
        store.capture(&existing).expect("capture");
        store.capture(&created).expect("capture");
        write(&created, "GENERATED");
        std::fs::write(&existing, "GENERATED").expect("write");

        let done = store.restore_all().expect("restore all");

        assert_eq!(done.len(), 2);
        assert!(store.is_empty());
        assert_eq!(std::fs::read_to_string(&existing).expect("read"), "KEEP");
        assert!(!created.exists());
    }

    #[test]
    fn restoring_something_it_never_owned_does_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut store = Store::open(&dir.path().join("state")).expect("open");
        assert_eq!(
            store.restore(Path::new("/etc/passwd")).expect("restore"),
            None
        );
    }
}
