//! Conformance against Omarchy 4.
//!
//! The expected files under `tests/fixtures` were produced by running the
//! upstream shell pipeline (`omarchy-theme-color`, `omarchy-theme-set-templates`)
//! at tag v4.0.0. They assert that `riso` is a drop-in replacement, not merely
//! that it is self-consistent.

use std::path::{Path, PathBuf};

use riso_core::palette::Palette;
use riso_core::template;

const RENDERED_THEMES: &[&str] = &["tokyo-night", "rose-pine", "catppuccin-latte"];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn resolved(theme: &str) -> Palette {
    let source = read(&fixtures().join("themes").join(format!("{theme}.toml")));
    let (palette, _) = Palette::parse(&source);
    let (palette, _) = palette.resolve(false);
    palette
}

fn theme_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures().join("themes"))
        .expect("themes fixture directory")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .path()
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

#[test]
fn resolves_every_shipped_palette_exactly_like_upstream() {
    let themes = theme_names();
    assert!(
        themes.len() >= 20,
        "expected the full theme set, got {}",
        themes.len()
    );

    for theme in themes {
        let expected = read(&fixtures().join("palette").join(format!("{theme}.tsv")));
        let actual: String = resolved(&theme)
            .iter()
            .map(|(key, value)| format!("{key}\t{value}\n"))
            .collect();

        assert_eq!(actual, expected, "palette mismatch for theme {theme}");
    }
}

#[test]
fn renders_every_shipped_template_exactly_like_upstream() {
    let templates: Vec<PathBuf> = std::fs::read_dir(fixtures().join("templates"))
        .expect("templates fixture directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert!(!templates.is_empty(), "expected template fixtures");

    for theme in RENDERED_THEMES {
        let palette = resolved(theme);
        for source in &templates {
            let name = source
                .file_stem()
                .expect("template stem")
                .to_string_lossy()
                .into_owned();
            let expected = read(&fixtures().join("rendered").join(theme).join(&name));
            let actual = template::render(&read(source), &palette);

            assert_eq!(actual, expected, "render mismatch for {name} on {theme}");
        }
    }
}

/// Every placeholder must be consumed: a leftover `{{ ... }}` in a generated
/// file is a hole the desktop would show.
#[test]
fn leaves_no_placeholder_behind_in_any_shipped_template() {
    let templates: Vec<PathBuf> = std::fs::read_dir(fixtures().join("templates"))
        .expect("templates fixture directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();

    for theme in theme_names() {
        let palette = resolved(&theme);
        for source in &templates {
            let rendered = template::render(&read(source), &palette);
            assert!(
                !rendered.contains("{{"),
                "unresolved placeholder rendering {} for {theme}",
                source.display()
            );
        }
    }
}
