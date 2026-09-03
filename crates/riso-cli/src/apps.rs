//! What the current configuration can theme, and where each piece comes
//! from. The listing runs the same resolution the renderer runs, dry:
//! the applied theme's own files first, then each template directory in
//! order, then the templates compiled into riso, plus the plugins
//! installed beside them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use riso_core::theme::{load_palette, render_theme, Options, Outcome};

#[derive(Debug, Serialize)]
pub struct App {
    pub name: String,
    pub source: String,
    pub detail: String,
}

/// Theme files that are the theme's own metadata, not application support.
const NOT_AN_APP: &[&str] = &["colors.toml", "light.mode"];

fn is_metadata(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    NOT_AN_APP.contains(&name)
        || upper.starts_with("LICENSE")
        || upper.starts_with("README")
        || upper.starts_with("NOTICE")
        || name.starts_with("preview.")
        || name.ends_with(".md")
}

fn stem(file: &str) -> &str {
    file.split('.').next().unwrap_or(file)
}

pub fn run(
    state_dir: &Path,
    theme_dirs: &[PathBuf],
    template_dirs: &[PathBuf],
    plugin_dirs: &[PathBuf],
) -> Result<Vec<App>, String> {
    // Keyed by (name, produced file): one app may own two spellings, like
    // hyprland.conf and hyprland.lua, and both deserve a row.
    let mut rows: BTreeMap<(String, String), App> = BTreeMap::new();

    // The applied theme, when there is one: its palette drives the dry
    // render, its own files are the strongest layer.
    let theme_name = std::fs::read_to_string(state_dir.join("current/theme.name"))
        .map(|n| n.trim().to_owned())
        .ok();
    let theme = theme_name.as_ref().and_then(|name| {
        riso_core::catalog::installed(theme_dirs, None)
            .into_iter()
            .find(|t| &t.name == name)
    });
    let palette = match &theme {
        Some(theme) => load_palette(&theme.path).map_err(|e| e.to_string())?.0,
        None => riso_core::Palette::parse("").0.resolve(false).0,
    };

    template_rows(&mut rows, &palette, template_dirs)?;
    if let (Some(name), Some(theme)) = (&theme_name, &theme) {
        theme_rows(&mut rows, name, &theme.path);
    }
    plugin_rows(&mut rows, plugin_dirs)?;
    Ok(rows.into_values().collect())
}

/// Every output the template layers would produce, each with the layer
/// that wins it, from a dry render over an empty target.
fn template_rows(
    rows: &mut BTreeMap<(String, String), App>,
    palette: &riso_core::Palette,
    template_dirs: &[PathBuf],
) -> Result<(), String> {
    let scratch = std::env::temp_dir().join(format!("riso-apps-{}", std::process::id()));
    let options = Options {
        template_dirs: template_dirs.to_vec(),
        dry_run: true,
        builtin: true,
        ..Default::default()
    };
    let report = render_theme(palette, &scratch, &options).map_err(|e| e.to_string())?;
    for outcome in &report.outcomes {
        let (Outcome::Rendered {
            template, target, ..
        }
        | Outcome::Kept { template, target }) = outcome;
        let Some(file) = target.file_name().map(|f| f.to_string_lossy().into_owned()) else {
            continue;
        };
        let source = if template.starts_with("<built-in>") {
            "riso built-in".to_owned()
        } else {
            let dir = template.parent().unwrap_or(Path::new(""));
            format!("templates {}", dir.display())
        };
        rows.insert(
            (stem(&file).to_owned(), file.clone()),
            App {
                name: stem(&file).to_owned(),
                source,
                detail: file,
            },
        );
    }
    Ok(())
}

/// The theme's own files, and its wallpapers as one row.
fn theme_rows(rows: &mut BTreeMap<(String, String), App>, name: &str, path: &Path) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(Result::ok) {
            let file = entry.file_name().to_string_lossy().into_owned();
            if !entry.path().is_file() || file.starts_with('.') || is_metadata(&file) {
                continue;
            }
            let key = (stem(&file).to_owned(), file.clone());
            let source = if rows.contains_key(&key) {
                format!("theme {name} (overrides a template)")
            } else {
                format!("theme {name}")
            };
            rows.insert(
                key.clone(),
                App {
                    name: key.0,
                    source,
                    detail: file,
                },
            );
        }
    }
    if let Ok(images) = std::fs::read_dir(path.join("backgrounds")) {
        let count = images.filter_map(Result::ok).count();
        if count > 0 {
            rows.insert(
                ("backgrounds".to_owned(), String::new()),
                App {
                    name: "backgrounds".to_owned(),
                    source: format!("theme {name}"),
                    detail: format!("{count} wallpapers"),
                },
            );
        }
    }
}

/// What the installed plugins add.
fn plugin_rows(
    rows: &mut BTreeMap<(String, String), App>,
    plugin_dirs: &[PathBuf],
) -> Result<(), String> {
    for plugin in riso_core::plugin::discover(plugin_dirs).map_err(|e| e.to_string())? {
        for render in &plugin.manifest.render {
            let file = render
                .target
                .rsplit('/')
                .next()
                .unwrap_or(&render.target)
                .to_owned();
            let key = (stem(&file).to_owned(), file.clone());
            rows.insert(
                key.clone(),
                App {
                    name: key.0,
                    source: format!("plugin {}", plugin.manifest.id),
                    detail: render.target.clone(),
                },
            );
        }
    }
    Ok(())
}

pub fn print(apps: &[App]) {
    let name_width = apps.iter().map(|a| a.name.len()).max().unwrap_or(0);
    let source_width = apps.iter().map(|a| a.source.len()).max().unwrap_or(0);
    for app in apps {
        println!(
            "{:<name_width$}  {:<source_width$}  {}",
            app.name, app.source, app.detail
        );
    }
}
