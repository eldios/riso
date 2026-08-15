use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use riso_core::apply::{apply, copy_tree, Request};
use riso_core::palette::Warning;
use riso_core::reload::ProcessExecutor;
use riso_core::theme::{load_palette, render_theme, Outcome};

#[derive(Parser)]
#[command(name = "riso", version, about = "Modular ricing framework")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply a theme: render it and hand it to the running desktop
    Set {
        /// Theme name; spaces and case do not matter
        name: String,
        /// Theme directory; repeat for more, later ones overlay earlier ones
        #[arg(long = "themes", value_name = "DIR", required = true)]
        theme_dirs: Vec<PathBuf>,
        /// Template directory; repeat for more, earlier ones take precedence
        #[arg(long = "templates", value_name = "DIR")]
        template_dirs: Vec<PathBuf>,
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        /// Do not notify the running desktop
        #[arg(long)]
        no_reload: bool,
    },
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

/// Where generated theme state lives, honouring XDG before falling back.
fn default_state_dir() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("omarchy"));
        }
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_owned())?;
    Ok(PathBuf::from(home).join(".local/state/omarchy"))
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Set {
            name,
            theme_dirs,
            template_dirs,
            state,
            no_reload,
        } => {
            let state_dir = match state {
                Some(dir) => dir,
                None => default_state_dir()?,
            };
            let request = Request {
                name,
                theme_dirs,
                template_dirs,
                state_dir,
                skip_reload: no_reload,
            };

            let applied = apply(&request, &ProcessExecutor).map_err(|e| e.to_string())?;
            report_warnings(&applied.warnings);

            println!(
                "applied {} to {} ({} rendered, {} from the theme)",
                applied.name,
                applied.target.display(),
                applied.report.rendered().count(),
                applied.report.kept().count()
            );
            Ok(())
        }
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
