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
    Catalog,
}

/// What picking an entry does.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Apply the pick to the desktop.
    Apply,
    /// Look, do not touch.
    Browse,
    /// Install the pick from the catalog.
    Install,
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

/// The catalog's themes, one row each, with previews fetched best effort.
///
/// A preview that cannot be downloaded is simply absent: browsing a catalog
/// must work on the index alone, and images arrive when they arrive. The
/// cache keeps one file per theme, refreshed only when missing.
pub fn catalog_rows(exec: &dyn riso_core::reload::Executor, index_url: &str) -> Vec<Row> {
    let Ok(index) = catalog::fetch_index(exec, index_url) else {
        return Vec::new();
    };
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .map(|base| base.join("riso/previews"));

    index
        .themes
        .iter()
        .filter(|entry| entry.yanked.is_none())
        .map(|entry| Row {
            label: entry.name.clone(),
            preview: entry
                .preview
                .as_deref()
                .zip(cache.as_deref())
                .and_then(|(url, cache)| fetch_preview(url, cache, &entry.name)),
            value: entry.name.clone(),
        })
        .collect()
}

fn fetch_preview(url: &str, cache: &Path, name: &str) -> Option<PathBuf> {
    let extension = url
        .rsplit('.')
        .next()
        .filter(|e| e.len() <= 4)
        .unwrap_or("img");
    let target = cache.join(format!("{name}.{extension}"));
    if target.is_file() {
        return Some(target);
    }
    std::fs::create_dir_all(cache).ok()?;
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "10", "-o"])
        .arg(&target)
        .arg(url)
        .status()
        .ok()?;
    status.success().then_some(target)
}

/// The wallpaper in use, when the link resolves.
pub fn current_background(state: &Path) -> Option<PathBuf> {
    riso_core::background::current(&state.join("current/background"))
}

/// `riso carousel-data`: the rows a picker reads, or what is current.
pub(crate) fn run(what: &str, current: bool) -> Result<(), String> {
    let state = crate::paths::default_state_dir()?;
    if current {
        match what {
            "backgrounds" => {
                if let Some(path) = current_background(&state) {
                    println!("{}", path.display());
                }
            }
            _ => {
                if let Some(name) = current_theme(&state) {
                    println!("{name}");
                }
            }
        }
        return Ok(());
    }
    let rows = match what {
        "backgrounds" => background_rows(&state),
        "catalog" => catalog_rows(
            &riso_core::reload::ProcessExecutor,
            crate::paths::DEFAULT_CATALOG,
        ),
        _ => theme_rows(),
    };
    for row in rows {
        let preview = row
            .preview
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        println!("{}\t{preview}\t{}", row.label, row.value);
    }
    Ok(())
}
