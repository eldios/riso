use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use riso_core::palette::Warning;
use riso_core::theme::{load_palette, render_theme, Outcome};

#[derive(Parser)]
#[command(name = "riso", version, about = "Modular ricing framework")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a theme's palette as resolved key/value pairs
    Palette {
        /// Directory holding colors.toml
        #[arg(long)]
        theme: PathBuf,
    },
    /// Build a theme into a directory of ready-to-read config files
    Render {
        /// Directory holding colors.toml and any hand-written theme files
        #[arg(long)]
        theme: PathBuf,
        /// Where the generated files are written
        #[arg(long)]
        out: PathBuf,
        /// Template directory; repeat for more, earlier ones take precedence
        #[arg(long = "templates", value_name = "DIR")]
        template_dirs: Vec<PathBuf>,
        /// Report what would be written without writing it
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("riso: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Palette { theme } => {
            let (palette, warnings) = load_palette(&theme).map_err(|e| e.to_string())?;
            report_warnings(&warnings);
            for (key, value) in palette.iter() {
                println!("{key}\t{value}");
            }
            Ok(())
        }
        Command::Render {
            theme,
            out,
            template_dirs,
            dry_run,
        } => {
            let (palette, warnings) = load_palette(&theme).map_err(|e| e.to_string())?;
            report_warnings(&warnings);

            if !dry_run {
                copy_tree(&theme, &out).map_err(|e| format!("copying the theme: {e}"))?;
            }

            let report =
                render_theme(&palette, &template_dirs, &out, dry_run).map_err(|e| e.to_string())?;

            for outcome in &report.outcomes {
                match outcome {
                    Outcome::Rendered { target, .. } => {
                        println!(
                            "{} {}",
                            if dry_run { "would write" } else { "wrote" },
                            target.display()
                        );
                    }
                    Outcome::Kept { target, .. } => {
                        println!("kept {} (provided by the theme)", target.display());
                    }
                }
            }
            Ok(())
        }
    }
}

/// Warnings go to stderr so they never contaminate piped output.
fn report_warnings(warnings: &[Warning]) {
    for warning in warnings {
        let text = match warning {
            Warning::UnsupportedKey { line } => {
                format!("line {line}: key has unsupported characters, skipped")
            }
            Warning::UnsupportedValue { line, key } => {
                format!("line {line}: value of '{key}' has unsupported characters, skipped")
            }
            Warning::EmptyValue { key } => {
                format!("'{key}' resolved to nothing, its placeholder will render empty")
            }
            Warning::UnderivableColor { key, source } => {
                format!("cannot derive '{key}': '{source}' is not a hex color")
            }
        };
        eprintln!("riso: {text}");
    }
}

/// Copy a theme into the output directory so hand-written files land before
/// any template runs and therefore win.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
