use std::path::{Path, PathBuf};
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
        /// Theme directory; repeat for more, later ones overlay earlier ones.
        /// Defaults to riso's search path.
        #[arg(long = "themes", value_name = "DIR")]
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
        /// Plugin directory; repeat for more, later ones override earlier ones
        #[arg(long = "plugins", value_name = "DIR")]
        plugin_dirs: Vec<PathBuf>,
    },
    /// Manage installed themes
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
    /// Manage plugins, which teach riso to theme more applications
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Put back everything riso wrote over
    Restore {
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        /// Restore one path instead of everything
        #[arg(long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Put everything back and forget the generated theme
    Uninstall {
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        /// Do it without asking
        #[arg(long)]
        yes: bool,
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
        /// Keep a theme the safety check would refuse. The findings still
        /// print; accepting them is on you.
        #[arg(long)]
        trust: bool,
    },
    /// Check that a theme is data and nothing else
    Validate {
        /// Theme directory to inspect
        path: PathBuf,
        /// Report findings without failing on them
        #[arg(long)]
        warn_only: bool,
    },
    /// Remove a theme riso installed
    Remove {
        name: String,
        /// Where themes were installed; defaults to the user theme directory
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List installed plugins
    List {
        #[arg(long = "plugins", value_name = "DIR")]
        plugin_dirs: Vec<PathBuf>,
    },
    /// Install a plugin from a git repository
    Install {
        /// Git URL
        repo: String,
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
        /// Install under this name instead of the one derived from the URL
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove an installed plugin
    Remove {
        name: String,
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
    },
}

fn run_plugin(action: PluginAction) -> Result<(), String> {
    match action {
        PluginAction::List { plugin_dirs } => {
            let dirs = if plugin_dirs.is_empty() {
                vec![user_plugin_dir()?]
            } else {
                plugin_dirs
            };
            let found = riso_core::plugin::discover(&dirs).map_err(|e| e.to_string())?;
            if found.is_empty() {
                eprintln!("riso: no plugins installed");
            }
            for plugin in found {
                println!(
                    "{}\t{}\t{} file(s)",
                    plugin.manifest.id,
                    plugin.manifest.name.as_deref().unwrap_or("-"),
                    plugin.manifest.render.len()
                );
            }
            Ok(())
        }
        PluginAction::Install { repo, into, name } => {
            let into = match into {
                Some(dir) => dir,
                None => user_plugin_dir()?,
            };
            let name = name.unwrap_or_else(|| catalog::name_from_repo(&repo));
            if !catalog::is_safe_name(&name) {
                return Err(format!("'{name}' is not usable as a directory name"));
            }
            // A plugin is code: cloning it is the moment to say so.
            eprintln!(
                "riso: a plugin runs as code on your machine; review {repo} before enabling it"
            );
            let path = catalog::install_from_git(
                &ProcessExecutor,
                &repo,
                None,
                &name,
                &into,
                "manifest.toml",
            )
            .map_err(|e| e.to_string())?;
            println!("installed {name} to {}", path.display());
            Ok(())
        }
        PluginAction::Remove { name, into } => {
            let into = match into {
                Some(dir) => dir,
                None => user_plugin_dir()?,
            };
            let path = into.join(&name);
            if !path.join("manifest.toml").is_file() {
                return Err(format!("nothing named '{name}' is installed"));
            }
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("removing {}: {e}", path.display()))?;
            println!("removed {}", path.display());
            Ok(())
        }
    }
}

/// Where `theme install` puts things, and the first place `set` looks.
const DEFAULT_CATALOG: &str =
    "https://raw.githubusercontent.com/eldios/riso-themes/main/index.json";

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME is not set".to_owned())
}

fn user_plugin_dir() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("riso/plugins"));
        }
    }
    Ok(home_dir()?.join(".config/riso/plugins"))
}

fn user_theme_dir() -> Result<PathBuf, String> {
    catalog::user_theme_dir().ok_or_else(|| "HOME is not set".to_owned())
}

/// Restore the built-in theme, reporting rather than failing: it is a way out
/// of having nothing, so refusing to continue would defeat it.
fn seed_fallback(dir: &Path) {
    match catalog::seed_fallback(dir) {
        Ok(path) => eprintln!(
            "riso: no themes found, wrote the built-in one to {}",
            path.display()
        ),
        Err(e) => eprintln!("riso: could not write the built-in theme: {e}"),
    }
}

fn run_theme(action: ThemeAction) -> Result<(), String> {
    match action {
        ThemeAction::List { theme_dirs } => {
            let user = user_theme_dir()?;
            let mut dirs = if theme_dirs.is_empty() {
                // installed() lets the first occurrence win, the reverse of
                // the overlay order the search path is written in.
                let mut defaults = catalog::default_theme_dirs();
                defaults.reverse();
                defaults
            } else {
                theme_dirs
            };
            if !dirs.contains(&user) {
                dirs.insert(0, user.clone());
            }

            let mut found = catalog::installed(&dirs, Some(&user));
            if found.is_empty() {
                seed_fallback(&user);
                found = catalog::installed(&dirs, Some(&user));
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
            trust,
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

            let path = catalog::install_from_git(
                &ProcessExecutor,
                &repo,
                rev.as_deref(),
                &name,
                &into,
                "colors.toml",
            )
            .map_err(|e| e.to_string())?;

            // The same gate the catalog runs, because this theme may never
            // have passed through one: a clone straight from a git URL is
            // exactly the case the client-side check exists for.
            let findings = riso_core::validate::validate(&path, &Default::default())
                .map_err(|e| e.to_string())?;
            let fatal = findings.iter().filter(|f| f.is_fatal()).count();
            for finding in &findings {
                eprintln!(
                    "riso: {} {}",
                    if finding.is_fatal() { "REFUSE" } else { "warn" },
                    finding.describe()
                );
            }
            if fatal > 0 && !trust {
                let _ = std::fs::remove_dir_all(&path);
                return Err(format!(
                    "'{name}' is not just data: {fatal} finding(s) make it unsafe, nothing was installed (--trust overrides)"
                ));
            }
            if fatal > 0 {
                eprintln!("riso: kept on your say-so: --trust accepted {fatal} finding(s)");
            }

            println!("installed {name} to {}", path.display());
            Ok(())
        }
        ThemeAction::Validate { path, warn_only } => {
            let findings = riso_core::validate::validate(&path, &Default::default())
                .map_err(|e| e.to_string())?;

            if findings.is_empty() {
                println!("{}: clean", path.display());
                return Ok(());
            }
            let fatal = findings.iter().filter(|f| f.is_fatal()).count();
            for finding in &findings {
                println!(
                    "{} {}",
                    if finding.is_fatal() {
                        "REFUSE"
                    } else {
                        "warn  "
                    },
                    finding.describe()
                );
            }
            if fatal > 0 && !warn_only {
                return Err(format!(
                    "{fatal} finding(s) make this theme unsafe to install"
                ));
            }
            Ok(())
        }
        ThemeAction::Remove { name, into } => {
            let into = match into {
                Some(dir) => dir,
                None => user_theme_dir()?,
            };
            let path = catalog::remove(&name, std::slice::from_ref(&into), &into)
                .map_err(|e| e.to_string())?;
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
///
/// A desktop that expects the generated theme somewhere else is told with
/// `--state`; the default is riso's own directory.
fn default_state_dir() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("riso"));
        }
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_owned())?;
    Ok(PathBuf::from(home).join(".local/state/riso"))
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
            plugin_dirs,
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
            let mut theme_dirs = if theme_dirs.is_empty() {
                catalog::default_theme_dirs()
            } else {
                theme_dirs
            };
            // A machine with no themes at all still applies one: the built-in
            // is written out as an ordinary theme rather than special-cased.
            if catalog::installed(&theme_dirs, None).is_empty() {
                if let Some(user) = catalog::user_theme_dir() {
                    seed_fallback(&user);
                    if !theme_dirs.contains(&user) {
                        theme_dirs.push(user);
                    }
                }
            }
            let request = Request {
                name,
                theme_dirs,
                template_dirs,
                state_dir,
                background_dirs: Vec::new(),
                hooks: Vec::new(),
                parts: Parts::default(),
                builtin_templates: !no_builtin,
                plugin_dirs: if plugin_dirs.is_empty() {
                    vec![user_plugin_dir()?]
                } else {
                    plugin_dirs
                },
                home: home_dir()?,
                desktop,
                // Omarchy publishes this; with it riso calls Quickshell
                // directly rather than the wrapper script.
                shell_config: std::env::var_os("OMARCHY_PATH")
                    .map(|p| PathBuf::from(p).join("shell"))
                    .filter(|p| p.is_dir()),
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
            for plugin in &applied.plugins {
                match &plugin.skipped {
                    Some(reason) => eprintln!("riso: skipped plugin {}: {reason}", plugin.id),
                    None => println!(
                        "plugin {} wrote {} file(s)",
                        plugin.id,
                        plugin.written.len()
                    ),
                }
            }
            Ok(())
        }
        Command::Restore { state, path } => {
            let state_dir = match state {
                Some(dir) => dir,
                None => default_state_dir()?,
            };
            let mut store = riso_core::snapshot::Store::open(&state_dir.join("ownership"))
                .map_err(|e| e.to_string())?;

            let done = match path {
                Some(path) => store
                    .restore(&path)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .collect(),
                None => store.restore_all().map_err(|e| e.to_string())?,
            };

            if done.is_empty() {
                eprintln!("riso: nothing to put back");
            }
            for outcome in done {
                match outcome {
                    riso_core::snapshot::Restored::Contents(path) => {
                        println!("restored {}", path.display())
                    }
                    riso_core::snapshot::Restored::Removed(path) => {
                        println!("removed {}", path.display())
                    }
                }
            }
            Ok(())
        }
        Command::Uninstall { state, yes } => {
            let state_dir = match state {
                Some(dir) => dir,
                None => default_state_dir()?,
            };
            let mut store = riso_core::snapshot::Store::open(&state_dir.join("ownership"))
                .map_err(|e| e.to_string())?;

            let owned = store.targets().count();
            if !yes {
                // Saying what will happen is the point: this is the command
                // someone runs when they want out.
                eprintln!(
                    "riso: this puts back {owned} file(s) and removes {}",
                    state_dir.display()
                );
                return Err("re-run with --yes to go ahead".to_owned());
            }

            for outcome in store.restore_all().map_err(|e| e.to_string())? {
                match outcome {
                    riso_core::snapshot::Restored::Contents(path) => {
                        println!("restored {}", path.display())
                    }
                    riso_core::snapshot::Restored::Removed(path) => {
                        println!("removed {}", path.display())
                    }
                }
            }
            std::fs::remove_dir_all(&state_dir)
                .map_err(|e| format!("removing {}: {e}", state_dir.display()))?;
            println!("removed {}", state_dir.display());
            Ok(())
        }
        Command::Theme { action } => run_theme(action),
        Command::Plugin { action } => run_plugin(action),
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
