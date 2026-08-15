//! Turning a theme directory into the files the desktop reads.
//!
//! The precedence rule is the whole point: a file already present in the output
//! is never overwritten by a template. That is what lets a theme hand-write one
//! file and let every other one be generated.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::atomic::write_atomic;
use crate::error::IoError;
use crate::palette::{Palette, Warning};
use crate::template;

const PALETTE_FILE: &str = "colors.toml";
const LIGHT_MODE_MARKER: &str = "light.mode";
const TEMPLATE_EXTENSION: &str = "tpl";

/// What happened to one output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Rendered {
        template: PathBuf,
        target: PathBuf,
    },
    /// The target already existed, so the template did not run.
    Kept {
        target: PathBuf,
        template: PathBuf,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
    pub warnings: Vec<Warning>,
    /// Section override files that were folded into `shell.toml`.
    pub section_overrides: Vec<PathBuf>,
}

impl Report {
    pub fn rendered(&self) -> impl Iterator<Item = &Outcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Rendered { .. }))
    }

    pub fn kept(&self) -> impl Iterator<Item = &Outcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Kept { .. }))
    }
}

/// Read and resolve `colors.toml` from a theme directory.
pub fn load_palette(theme_dir: &Path) -> Result<(Palette, Vec<Warning>), IoError> {
    let path = theme_dir.join(PALETTE_FILE);
    let raw = std::fs::read_to_string(&path).map_err(|e| IoError::Read(path.clone(), e))?;

    let (palette, mut warnings) = Palette::parse(&raw);
    let (palette, resolution_warnings) =
        palette.resolve(theme_dir.join(LIGHT_MODE_MARKER).exists());
    warnings.extend(resolution_warnings);

    Ok((palette, warnings))
}

/// Render every template into `out_dir`.
///
/// `template_dirs` are searched in order and the first one to claim an output
/// name wins, so caller-supplied directories should come before built-in ones.
///
/// `only` restricts rendering to those output names; empty renders every
/// template.
pub fn render_theme(
    palette: &Palette,
    template_dirs: &[PathBuf],
    out_dir: &Path,
    dry_run: bool,
    only: &[String],
) -> Result<Report, IoError> {
    let mut report = Report::default();
    let mut claimed: BTreeMap<String, ()> = BTreeMap::new();

    for dir in template_dirs {
        for path in templates_in(dir)? {
            let Some(name) = output_name(&path) else {
                continue;
            };
            if !only.is_empty() && !only.contains(&name) {
                continue;
            }
            if claimed.contains_key(&name) {
                continue;
            }
            claimed.insert(name.clone(), ());

            let target = out_dir.join(&name);
            if target.exists() {
                report.outcomes.push(Outcome::Kept {
                    target,
                    template: path,
                });
                continue;
            }

            let source =
                std::fs::read_to_string(&path).map_err(|e| IoError::Read(path.clone(), e))?;
            let rendered = template::render(&source, palette);
            if !dry_run {
                write_atomic(&target, &rendered)?;
            }
            report.outcomes.push(Outcome::Rendered {
                template: path,
                target,
            });
        }
    }

    // Section overrides run last: they edit the shell.toml the loop just wrote.
    if !dry_run {
        report.section_overrides = crate::section::apply_overrides(out_dir)?;
    }

    Ok(report)
}

/// Templates in a directory, sorted so a run is reproducible.
///
/// A missing directory is empty rather than an error: the user template
/// directory is optional by design.
fn templates_in(dir: &Path) -> Result<Vec<PathBuf>, IoError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(IoError::Read(dir.into(), e)),
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == TEMPLATE_EXTENSION)
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// `foo.conf.tpl` produces `foo.conf`.
fn output_name(template: &Path) -> Option<String> {
    Some(template.file_stem()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        let (palette, _) = Palette::parse("background = \"#1a1b26\"\n");
        let (palette, _) = palette.resolve(false);
        palette
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, contents).expect("write");
    }

    #[test]
    fn renders_a_template_into_the_output_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let templates = dir.path().join("templates");
        let out = dir.path().join("out");
        write(&templates.join("app.conf.tpl"), "bg={{ background }}");

        let report = render_theme(&palette(), &[templates], &out, false, &[]).expect("render");

        assert_eq!(
            std::fs::read_to_string(out.join("app.conf")).expect("read"),
            "bg=#1a1b26"
        );
        assert_eq!(report.rendered().count(), 1);
    }

    #[test]
    fn never_overwrites_a_file_the_theme_already_provided() {
        let dir = tempfile::tempdir().expect("tempdir");
        let templates = dir.path().join("templates");
        let out = dir.path().join("out");
        write(&templates.join("app.conf.tpl"), "generated");
        write(&out.join("app.conf"), "hand written");

        let report = render_theme(&palette(), &[templates], &out, false, &[]).expect("render");

        assert_eq!(
            std::fs::read_to_string(out.join("app.conf")).expect("read"),
            "hand written"
        );
        assert_eq!(report.kept().count(), 1);
        assert_eq!(report.rendered().count(), 0);
    }

    #[test]
    fn an_earlier_directory_claims_the_output_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user");
        let builtin = dir.path().join("builtin");
        let out = dir.path().join("out");
        write(&user.join("app.conf.tpl"), "from user");
        write(&builtin.join("app.conf.tpl"), "from builtin");

        render_theme(&palette(), &[user, builtin], &out, false, &[]).expect("render");

        assert_eq!(
            std::fs::read_to_string(out.join("app.conf")).expect("read"),
            "from user"
        );
    }

    #[test]
    fn dry_run_reports_without_writing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let templates = dir.path().join("templates");
        let out = dir.path().join("out");
        write(&templates.join("app.conf.tpl"), "bg={{ background }}");

        let report = render_theme(&palette(), &[templates], &out, true, &[]).expect("render");

        assert_eq!(report.rendered().count(), 1);
        assert!(!out.join("app.conf").exists());
    }

    #[test]
    fn a_missing_template_directory_is_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report = render_theme(
            &palette(),
            &[dir.path().join("absent")],
            &dir.path().join("out"),
            false,
            &[],
        )
        .expect("render");
        assert_eq!(report.outcomes.len(), 0);
    }

    #[test]
    fn loads_a_palette_and_honours_the_light_mode_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            &dir.path().join("colors.toml"),
            "background = \"#000000\"\n",
        );
        let (palette, _) = load_palette(dir.path()).expect("load");
        assert_eq!(palette.get("mode").as_deref(), Some("dark"));

        write(&dir.path().join("light.mode"), "");
        let (palette, _) = load_palette(dir.path()).expect("load");
        assert_eq!(palette.get("mode").as_deref(), Some("light"));
    }
}
