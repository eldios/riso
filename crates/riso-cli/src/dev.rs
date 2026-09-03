//! `riso dev`: the theme author's tools, palette and standalone render.

use std::path::PathBuf;

use riso_core::apply::copy_tree;
use riso_core::theme::{load_palette, render_theme, Options as RenderOptions, Outcome};

use crate::cli::DevAction;
use crate::output::{emit, report_warnings, OutputFormat};

pub(crate) fn run(action: DevAction, output: OutputFormat) -> Result<(), String> {
    match action {
        DevAction::Palette { theme } => palette(&theme, output),
        DevAction::Render {
            theme,
            out,
            template_dirs,
            dry_run,
            no_builtin,
        } => render(&theme, &out, template_dirs, dry_run, no_builtin, output),
    }
}

fn palette(theme: &std::path::Path, output: OutputFormat) -> Result<(), String> {
    let (palette, warnings) = load_palette(theme).map_err(|e| e.to_string())?;
    report_warnings(&warnings);
    let pairs: Vec<_> = palette
        .iter()
        .map(|(key, value)| serde_json::json!({ "key": key, "value": value }))
        .collect();
    if !emit(output, &pairs)? {
        for pair in &pairs {
            println!(
                "{}\t{}",
                pair["key"].as_str().unwrap_or_default(),
                pair["value"].as_str().unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn render(
    theme: &std::path::Path,
    out: &std::path::Path,
    template_dirs: Vec<PathBuf>,
    dry_run: bool,
    no_builtin: bool,
    output: OutputFormat,
) -> Result<(), String> {
    let (palette, warnings) = load_palette(theme).map_err(|e| e.to_string())?;
    report_warnings(&warnings);
    if !dry_run {
        copy_tree(theme, out).map_err(|e| format!("copying the theme: {e}"))?;
    }
    let options = RenderOptions {
        template_dirs,
        dry_run,
        builtin: !no_builtin,
        ..Default::default()
    };
    let report = render_theme(&palette, out, &options).map_err(|e| e.to_string())?;

    let (written, kept): (Vec<_>, Vec<_>) = report
        .outcomes
        .iter()
        .partition(|o| matches!(o, Outcome::Rendered { .. }));
    let target = |o: &Outcome| match o {
        Outcome::Rendered { target, .. } | Outcome::Kept { target, .. } => target.clone(),
    };
    let written: Vec<PathBuf> = written.into_iter().map(target).collect();
    let kept: Vec<PathBuf> = kept.into_iter().map(target).collect();

    let summary = serde_json::json!({ "dry_run": dry_run, "written": written, "kept": kept });
    if !emit(output, &summary)? {
        let verb = if dry_run { "would write" } else { "wrote" };
        for target in &written {
            println!("{verb} {}", target.display());
        }
        for target in &kept {
            println!("kept {} (provided by the theme)", target.display());
        }
    }
    Ok(())
}
