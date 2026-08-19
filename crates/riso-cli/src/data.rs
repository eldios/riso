//! Rows for the pickers: label, preview image and the value applying takes.
//!
//! The GUI carousel and the TUI show the same catalog, so both read it here,
//! and the hidden carousel-data command is a thin pipe over the same rows.

use std::path::{Path, PathBuf};

use riso_core::catalog;

/// What a picker browses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum What {
    Themes,
    Backgrounds,
}

pub struct Row {
    pub label: String,
    pub preview: Option<PathBuf>,
    pub value: String,
}

/// The file a theme names preview.*, else its first background.
pub fn theme_preview(theme: &Path) -> Option<PathBuf> {
    for name in ["preview.png", "preview.jpg", "preview.jpeg", "preview.webp"] {
        let candidate = theme.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    riso_core::background::candidates(&[theme.join("backgrounds")])
        .into_iter()
        .next()
}

/// Every installed theme, one row each.
pub fn theme_rows() -> Vec<Row> {
    let dirs = catalog::default_theme_dirs();
    catalog::installed(&dirs, None)
        .into_iter()
        .map(|theme| Row {
            label: theme.name.clone(),
            preview: theme_preview(&theme.path),
            value: theme.name,
        })
        .collect()
}

/// The current theme's wallpapers, one row each; the image is its own preview.
pub fn background_rows(state: &Path) -> Vec<Row> {
    riso_core::background::candidates(&[state.join("current/theme/backgrounds")])
        .into_iter()
        .map(|image| Row {
            label: image
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            preview: Some(image.clone()),
            value: image.to_string_lossy().into_owned(),
        })
        .collect()
}

/// The name of the theme in use, when one is recorded.
pub fn current_theme(state: &Path) -> Option<String> {
    let name = std::fs::read_to_string(state.join("current/theme.name")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The wallpaper in use, when the link resolves.
pub fn current_background(state: &Path) -> Option<PathBuf> {
    riso_core::background::current(&state.join("current/background"))
}
