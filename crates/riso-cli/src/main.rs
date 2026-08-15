use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use riso_core::apply::{apply, copy_tree, Parts, Request};
use riso_core::catalog;
use riso_core::desktop::Desktop;
use riso_core::palette::Warning;
use riso_core::reload::ProcessExecutor;
use riso_core::theme::{load_palette, render_theme, Options as RenderOptions, Outcome};

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
        /// Ignore the templates compiled into riso
        #[arg(long)]
        no_builtin: bool,
        /// Which desktop to notify: omarchy, hyprland, sway, niri, none.
        /// Detected from the session when not given.
        #[arg(long)]
        desktop: Option<String>,
    },
    /// Manage installed themes
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
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
        /// Ignore the templates compiled into riso
        #[arg(long)]
        no_builtin: bool,
    },
}

#[derive(Subcommand)]
enum ThemeAction {
    /// List the themes riso can see
    List {
        /// Theme directory; repeat for more
        #[arg(long = "themes", value_name = "DIR")]
        theme_dirs: Vec<PathBuf>,
    },
    /// Install a theme from the catalog or from any git repository
    Install {
        /// Theme name in the catalog, or a git URL
        source: String,
        /// Where to install; defaults to the user theme directory
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
        /// Catalog index to look the name up in
        #[arg(long, value_name = "URL", default_value = DEFAULT_CATALOG)]
        catalog: String,
        /// Install under this name instead of the one derived from the source
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a theme riso installed
    Remove {
        name: String,
        /// Where themes were installed; defaults to the user theme directory
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
    },
}

/// Where `theme install` puts things, and the first place `set` looks.
const DEFAULT_CATALOG: &str =
    "https://raw.githubusercontent.com/eldios/riso-themes/main/index.json";

fn user_theme_dir() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("riso/themes"));
        }
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_owned())?;
    Ok(PathBuf::from(home).join(".config/riso/themes"))
}

fn run_theme(action: ThemeAction) -> Result<(), String> {
    match action {
        ThemeAction::List { theme_dirs } => {
            let mut dirs = theme_dirs;
            let user = user_theme_dir()?;
            if !dirs.contains(&user) {
                dirs.insert(0, user.clone());
            }
            let found = catalog::installed(&dirs, Some(&user));
            if found.is_empty() {
                eprintln!("riso: no themes found in {} directories", dirs.len());
            }
            for theme in found {
                println!(
                    "{}	{}{}",
                    theme.name,
                    theme.path.display(),
                    if theme.removable { "" } else { "	(read-only)" }
                );
            }
            Ok(())
        }
        ThemeAction::Install {
            source,
            into,
            catalog: index_url,
            name,
        } => {
            let into = match into {
                Some(dir) => dir,
                None => user_theme_dir()?,
            };
            let looks_like_url = source.contains("://") || source.contains('@');

            let (repo, rev) = if looks_like_url {
                (source.clone(), None)
            } else {
                let index = catalog::fetch_index(&ProcessExecutor, &index_url)
                    .map_err(|e| e.to_string())?;
                let entry = index
                    .find(&source)
                    .ok_or_else(|| format!("no theme named '{source}' in the catalog"))?;
                if let Some(reason) = &entry.yanked {
                    return Err(format!(
                        "'{source}' was withdrawn from the catalog: {reason}"
                    ));
                }
                (entry.repo.clone(), entry.rev.clone())
            };

            let name = name.unwrap_or_else(|| catalog::name_from_repo(&repo));
            if !catalog::is_safe_name(&name) {
                return Err(format!("'{name}' is not usable as a directory name"));
            }

            let path =
                catalog::install_from_git(&ProcessExecutor, &repo, rev.as_deref(), &name, &into)
                    .map_err(|e| e.to_string())?;

            println!("installed {name} to {}", path.display());
            Ok(())
        }
        ThemeAction::Remove { name, into } => {
            let into = match into {
                Some(dir) => dir,
                None => user_theme_dir()?,
            };
            let path = catalog::remove(&name, std::slice::from_ref(&into), &into).map_err(|e| e.to_string())?;
            println!("removed {}", path.display());
            Ok(())
        }
    }
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
            no_builtin,
            desktop,
        } => {
            let state_dir = match state {
                Some(dir) => dir,
                None => default_state_dir()?,
            };
            let desktop = match desktop {
                Some(name) => {
                    Desktop::from_name(&name).ok_or_else(|| format!("unknown desktop '{name}'"))?
                }
                None => Desktop::detect(),
            };
            let request = Request {
                name,
                theme_dirs,
                template_dirs,
                state_dir,
                background_dirs: Vec::new(),
                hooks: Vec::new(),
                parts: Parts::default(),
                builtin_templates: !no_builtin,
                desktop,
                skip_reload: no_reload,
            };

            let applied = apply(&request, &ProcessExecutor).map_err(|e| e.to_string())?;
            report_warnings(&applied.warnings);

            println!(
                "applied {} to {} ({} rendered, {} from the theme, desktop: {})",
                applied.name,
                applied.target.display(),
                applied.report.rendered().count(),
                applied.report.kept().count(),
                desktop.name()
            );
            if let Some(background) = &applied.background {
                println!("background {}", background.display());
            }
            Ok(())
        }
        Command::Theme { action } => run_theme(action),
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
            no_builtin,
        } => {
            let (palette, warnings) = load_palette(&theme).map_err(|e| e.to_string())?;
            report_warnings(&warnings);

            if !dry_run {
                copy_tree(&theme, &out).map_err(|e| format!("copying the theme: {e}"))?;
            }

            let report = render_theme(
                &palette,
                &out,
                &RenderOptions {
                    template_dirs,
                    dry_run,
                    builtin: !no_builtin,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;

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
