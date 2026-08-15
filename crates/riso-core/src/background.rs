//! Choosing the desktop background that belongs to a theme.
//!
//! A theme ships a directory of images and the user may add more. Selection
//! cycles: applying the same theme twice moves to the next image, which is how
//! one theme offers several looks without becoming several themes.

use std::path::{Path, PathBuf};

use crate::error::IoError;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp"];

/// Images available for a theme, sorted, from every directory that has any.
pub fn candidates(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut images: Vec<PathBuf> = dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_image(path))
        .collect();
    images.sort();
    images
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lowered = ext.to_ascii_lowercase();
            IMAGE_EXTENSIONS.contains(&lowered.as_str())
        })
        .unwrap_or(false)
}

/// The image to show next, given what is showing now.
///
/// Cycling from the current one is what makes a second apply advance the
/// wallpaper; an unknown current image starts the list over.
pub fn next(images: &[PathBuf], current: Option<&Path>) -> Option<PathBuf> {
    if images.is_empty() {
        return None;
    }
    let index = current
        .and_then(|current| images.iter().position(|image| image == current))
        .map(|position| (position + 1) % images.len())
        .unwrap_or(0);
    images.get(index).cloned()
}

/// Point `link` at `target`, replacing whatever it pointed at.
///
/// The link is built beside its destination and renamed over it, so a reader
/// never finds it missing.
pub fn link(link: &Path, target: &Path) -> Result<(), IoError> {
    let parent = link
        .parent()
        .ok_or_else(|| IoError::NoParent(link.into()))?;
    std::fs::create_dir_all(parent).map_err(|e| IoError::Write(parent.into(), e))?;

    let staging = link.with_file_name(format!(
        ".{}.riso-tmp",
        link.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "background".to_owned())
    ));
    let _ = std::fs::remove_file(&staging);

    std::os::unix::fs::symlink(target, &staging).map_err(|e| IoError::Write(staging.clone(), e))?;
    std::fs::rename(&staging, link).map_err(|e| {
        let _ = std::fs::remove_file(&staging);
        IoError::Write(link.into(), e)
    })
}

/// What `link` currently points at, if anything.
pub fn current(link: &Path) -> Option<PathBuf> {
    std::fs::read_link(link).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, "").expect("write");
    }

    #[test]
    fn collects_images_from_every_directory_sorted() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(&dir.path().join("theme/b.png"));
        touch(&dir.path().join("theme/a.jpg"));
        touch(&dir.path().join("user/c.JPEG"));
        touch(&dir.path().join("theme/notes.txt"));

        let found = candidates(&[dir.path().join("theme"), dir.path().join("user")]);

        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.jpg", "b.png", "c.JPEG"]);
    }

    #[test]
    fn a_missing_directory_contributes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(candidates(&[dir.path().join("absent")]).is_empty());
    }

    #[test]
    fn cycles_to_the_following_image() {
        let images: Vec<PathBuf> = ["a.png", "b.png", "c.png"]
            .iter()
            .map(PathBuf::from)
            .collect();

        assert_eq!(
            next(&images, Some(Path::new("a.png"))),
            Some(PathBuf::from("b.png"))
        );
        assert_eq!(
            next(&images, Some(Path::new("c.png"))),
            Some(PathBuf::from("a.png"))
        );
    }

    #[test]
    fn starts_over_when_the_current_image_is_unknown() {
        let images: Vec<PathBuf> = ["a.png", "b.png"].iter().map(PathBuf::from).collect();

        assert_eq!(next(&images, None), Some(PathBuf::from("a.png")));
        assert_eq!(
            next(&images, Some(Path::new("gone.png"))),
            Some(PathBuf::from("a.png"))
        );
    }

    #[test]
    fn offers_nothing_when_the_theme_ships_no_image() {
        assert_eq!(next(&[], None), None);
    }

    #[test]
    fn replaces_an_existing_link_without_a_gap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = dir.path().join("first.png");
        let second = dir.path().join("second.png");
        touch(&first);
        touch(&second);
        let pointer = dir.path().join("state/background");

        link(&pointer, &first).expect("first link");
        assert_eq!(current(&pointer).as_deref(), Some(first.as_path()));

        link(&pointer, &second).expect("second link");
        assert_eq!(current(&pointer).as_deref(), Some(second.as_path()));

        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("state"))
            .expect("readdir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("riso-tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temporary link may survive");
    }

    #[test]
    fn reports_no_current_image_when_the_link_is_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(current(&dir.path().join("background")), None);
    }
}
