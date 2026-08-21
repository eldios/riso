use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod check;
mod data;
mod gui;
mod output;
mod tui;

use output::{emit, OutputFormat};

use riso_core::apply::{apply, copy_tree, Parts, Request};
use riso_core::catalog;
use riso_core::desktop::Desktop;
use riso_core::palette::Warning;
use riso_core::reload::ProcessExecutor;
use riso_core::theme::{load_palette, render_theme, Options as RenderOptions, Outcome};

#[derive(Parser)]
#[command(name = "riso", version, about = "Modular ricing framework")]
struct Cli {
    /// Output format for results; defaults to `output` in config.toml
    #[arg(short = 'o', long = "output", global = true, value_enum)]
    output: Option<OutputFormat>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage themes: apply, list, install, validate, remove
    #[command(visible_alias = "t")]
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
    /// Change the wallpaper: the link, and the desktop that draws it
    #[command(name = "backgrounds", visible_alias = "b")]
    Backgrounds {
        #[command(subcommand)]
        action: BgAction,
    },
    /// Manage plugins, which teach riso to theme more applications
    #[command(visible_alias = "p")]
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Tools for theme authors
    #[command(visible_alias = "d")]
    Dev {
        #[command(subcommand)]
        action: DevAction,
    },
    /// Read and change riso's few options, kept in config.toml
    #[command(
        visible_alias = "c",
        after_help = "The options live in ~/.config/riso/config.toml and stay few on purpose.\n\
                      Everything situational is a flag: see riso(1) or the project README."
    )]
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
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
    /// Rows for the pickers: label, preview and value, tab-separated
    #[command(hide = true)]
    CarouselData {
        #[arg(value_parser = ["themes", "backgrounds", "catalog"])]
        what: String,
        /// Print what is current instead of the rows
        #[arg(long)]
        current: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print every option and its value
    #[command(visible_alias = "l")]
    List,
    /// Can this system carry riso: tools, desktop, and include wiring
    #[command(visible_alias = "k")]
    Check {
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        /// Check as this desktop instead of detecting one:
        /// omarchy, hyprland, sway, niri, none
        #[arg(long)]
        desktop: Option<String>,
    },
    /// Print one option's value
    #[command(visible_alias = "g")]
    Get { key: String },
    /// Change an option
    #[command(visible_alias = "s")]
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum BgAction {
    /// Use this image, or pick one with --gui/--tui
    #[command(visible_alias = "s")]
    Set {
        /// Image file; omit it to pick with --gui or --tui
        image: Option<PathBuf>,
        /// Pick from the full-screen carousel (needs quickshell)
        #[arg(long, conflicts_with_all = ["image", "tui"])]
        gui: bool,
        /// Pick from a picker drawn in the terminal
        #[arg(long, conflicts_with = "image")]
        tui: bool,
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        /// Do not tell the running desktop
        #[arg(long)]
        no_reload: bool,
    },
    /// Advance to the current theme's next background
    #[command(visible_alias = "n")]
    Next {
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        #[arg(long)]
        no_reload: bool,
    },
    /// Set or, with no argument, print how the wallpaper is scaled
    #[command(visible_alias = "m")]
    Mode {
        #[arg(value_parser = ["fill", "fit", "center", "stretch", "tile"])]
        mode: Option<String>,
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
    },
    /// Print the wallpaper in use and its mode
    #[command(visible_alias = "g")]
    Get {
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ThemeAction {
    /// Apply a theme: render it and hand it to the running desktop
    #[command(visible_alias = "s")]
    Set {
        /// Theme name; spaces and case do not matter. Omit it to pick
        /// with --gui or --tui.
        name: Option<String>,
        /// Pick from the full-screen carousel (needs quickshell)
        #[arg(long, conflicts_with_all = ["name", "tui"])]
        gui: bool,
        /// Pick from a picker drawn in the terminal
        #[arg(long, conflicts_with = "name")]
        tui: bool,
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
    /// Print the theme in use
    #[command(visible_alias = "g")]
    Get {
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
    },
    /// List the themes riso can see
    #[command(visible_alias = "l")]
    List {
        /// Theme directory; repeat for more
        #[arg(long = "themes", value_name = "DIR")]
        theme_dirs: Vec<PathBuf>,
        /// Browse previews in the full-screen carousel, read-only
        #[arg(long, conflicts_with = "tui")]
        gui: bool,
        /// Browse previews in the terminal, read-only
        #[arg(long)]
        tui: bool,
    },
    /// Install a theme from the catalog or from any git repository
    #[command(visible_alias = "i")]
    Install {
        /// Theme name in the catalog, or a git URL. Omit it to browse the
        /// catalog with --gui or --tui.
        source: Option<String>,
        /// Browse the catalog in the full-screen carousel; Enter installs
        #[arg(long, conflicts_with_all = ["source", "tui"])]
        gui: bool,
        /// Browse the catalog in the terminal; Enter installs
        #[arg(long, conflicts_with = "source")]
        tui: bool,
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
    /// Update installed themes from where they came from
    #[command(visible_alias = "u")]
    Update {
        /// One theme instead of all of them
        name: Option<String>,
        /// Where themes were installed; defaults to the user theme directory
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
        /// Catalog index that pins revisions for the themes it carries
        #[arg(long, value_name = "URL", default_value = DEFAULT_CATALOG)]
        catalog: String,
        /// Keep an update the safety check would refuse
        #[arg(long)]
        trust: bool,
        /// Print nothing; the exit code alone says how it went
        #[arg(short, long)]
        quiet: bool,
    },
    /// Check that a theme is data and nothing else
    #[command(visible_alias = "v")]
    Validate {
        /// Theme directory to inspect
        path: PathBuf,
        /// Report findings without failing on them
        #[arg(long)]
        warn_only: bool,
    },
    /// Remove a theme riso installed
    #[command(visible_alias = "rm")]
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
    #[command(visible_alias = "l")]
    List {
        #[arg(long = "plugins", value_name = "DIR")]
        plugin_dirs: Vec<PathBuf>,
    },
    /// Install a plugin from a git repository
    #[command(visible_alias = "i")]
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
    #[command(visible_alias = "rm")]
    Remove {
        name: String,
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DevAction {
    /// Print a theme's palette as resolved key/value pairs
    #[command(visible_alias = "p")]
    Palette {
        /// Directory holding colors.toml
        #[arg(long)]
        theme: PathBuf,
    },
    /// Build a theme into a directory of ready-to-read config files
    #[command(visible_alias = "r")]
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

fn run_plugin(action: PluginAction, output: OutputFormat) -> Result<(), String> {
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
            let rows: Vec<_> = found
                .iter()
                .map(|plugin| {
                    serde_json::json!({
                        "id": plugin.manifest.id,
                        "name": plugin.manifest.name,
                        "files": plugin.manifest.render.len(),
                    })
                })
                .collect();
            if !emit(output, &rows)? {
                for plugin in &found {
                    println!(
                        "{}\t{}\t{} file(s)",
                        plugin.manifest.id,
                        plugin.manifest.name.as_deref().unwrap_or("-"),
                        plugin.manifest.render.len()
                    );
                }
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
            if !emit(
                output,
                &serde_json::json!({ "installed": name, "path": path }),
            )? {
                println!("installed {name} to {}", path.display());
            }
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
            if !emit(output, &serde_json::json!({ "removed": path }))? {
                println!("removed {}", path.display());
            }
            Ok(())
        }
    }
}

/// Where `theme install` puts things, and the first place `set` looks.
pub(crate) const DEFAULT_CATALOG: &str = "https://catalog.riso.re/index.json";

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

fn run_theme(action: ThemeAction, output: OutputFormat) -> Result<(), String> {
    let _ = output;
    match action {
        ThemeAction::Set {
            name,
            gui,
            tui,
            theme_dirs,
            template_dirs,
            state,
            no_reload,
            no_builtin,
            desktop,
            plugin_dirs,
        } => {
            if gui {
                return gui::run(data::What::Themes, data::Purpose::Apply);
            }
            if tui {
                return tui::run(data::What::Themes, data::Purpose::Apply, state);
            }
            let Some(name) = name else {
                return Err("a theme name is needed, or --gui/--tui to pick one".to_owned());
            };
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
            // On an Omarchy system its template layers apply by default:
            // the user's own, then the desktop's. Explicit flags replace
            // them entirely.
            let template_dirs = if template_dirs.is_empty() {
                match std::env::var_os("OMARCHY_PATH").filter(|v| !v.is_empty()) {
                    Some(omarchy) => {
                        let mut dirs = Vec::new();
                        if let Some(home) = std::env::var_os("HOME") {
                            dirs.push(PathBuf::from(home).join(".config/omarchy/themed"));
                        }
                        dirs.push(PathBuf::from(omarchy).join("default/themed"));
                        dirs
                    }
                    None => template_dirs,
                }
            } else {
                template_dirs
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

            let summary = serde_json::json!({
                "theme": applied.name,
                "target": applied.target,
                "rendered": applied.report.rendered().count(),
                "kept": applied.report.kept().count(),
                "desktop": desktop.name(),
                "background": applied.background,
            });
            if !emit(output, &summary)? {
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
        ThemeAction::Get { state } => {
            let state = state_or_default(state)?;
            match data::current_theme(&state) {
                Some(name) => {
                    if !emit(output, &serde_json::json!({ "name": name }))? {
                        println!("{name}");
                    }
                    Ok(())
                }
                None => Err("no theme is applied".to_owned()),
            }
        }
        ThemeAction::List {
            theme_dirs,
            gui,
            tui,
        } => {
            if gui {
                return gui::run(data::What::Themes, data::Purpose::Browse);
            }
            if tui {
                return tui::run(data::What::Themes, data::Purpose::Browse, None);
            }
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
            let rows: Vec<_> = found
                .into_iter()
                .map(|theme| {
                    serde_json::json!({
                        "name": theme.name,
                        "path": theme.path,
                        "read_only": !theme.removable,
                    })
                })
                .collect();
            if !emit(output, &rows)? {
                for row in &rows {
                    println!(
                        "{}	{}{}",
                        row["name"].as_str().unwrap_or_default(),
                        row["path"].as_str().unwrap_or_default(),
                        if row["read_only"] == true {
                            "	(read-only)"
                        } else {
                            ""
                        }
                    );
                }
            }
            Ok(())
        }
        ThemeAction::Install {
            source,
            gui,
            tui,
            into,
            catalog: index_url,
            name,
            trust,
        } => {
            if gui {
                return gui::run(data::What::Catalog, data::Purpose::Install);
            }
            if tui {
                return tui::run(data::What::Catalog, data::Purpose::Install, None);
            }
            let Some(source) = source else {
                return Err(
                    "a theme name or git URL is needed, or --gui/--tui to browse".to_owned(),
                );
            };
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
            // exactly the case the client-side check exists for. Only the
            // findings that make a theme unsafe matter here; catalog policy
            // (license, size) belongs to `theme validate` and the CI that
            // runs it.
            let findings = riso_core::validate::validate(&path, &Default::default())
                .map_err(|e| e.to_string())?;
            let fatal = findings.iter().filter(|f| f.is_fatal()).count();
            for finding in findings.iter().filter(|f| f.is_fatal()) {
                eprintln!("riso: REFUSE {name}: {}", finding.describe());
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

            if !emit(
                output,
                &serde_json::json!({ "installed": name, "path": path }),
            )? {
                println!("installed {name} to {}", path.display());
            }
            Ok(())
        }
        ThemeAction::Update {
            name,
            into,
            catalog: index_url,
            trust,
            quiet,
        } => {
            let into = match into {
                Some(dir) => dir,
                None => user_theme_dir()?,
            };
            // The catalog is a hint, not a gate: a theme it does not carry
            // updates from its own origin. Failing to fetch it only means
            // no revisions are pinned.
            let index = catalog::fetch_index(&ProcessExecutor, &index_url).ok();

            let mut targets: Vec<PathBuf> = Vec::new();
            match &name {
                Some(name) => {
                    let path = into.join(name);
                    if !path.is_dir() {
                        return Err(format!("nothing named '{name}' is installed"));
                    }
                    targets.push(path);
                }
                None => {
                    for theme in catalog::installed(std::slice::from_ref(&into), Some(&into)) {
                        targets.push(theme.path);
                    }
                }
            }

            // Three voices: -q says nothing and lets the exit code speak,
            // -o json/yaml keeps the machine report, and plain human mode
            // narrates each theme as it happens.
            let verbose = !quiet && output == OutputFormat::Human;
            let total = targets.len();
            let short = |rev: &str| rev[..rev.len().min(7)].to_owned();
            if verbose {
                println!(
                    "updating {total} theme{} in {}",
                    if total == 1 { "" } else { "s" },
                    into.display()
                );
            }

            let mut report = Vec::new();
            let (mut updated, mut current, mut skipped, mut failed) = (0, 0, 0, 0);
            for (i, path) in targets.iter().enumerate() {
                let theme = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let rev = index
                    .as_ref()
                    .and_then(|i| i.find(&theme))
                    .and_then(|e| e.rev.as_deref().map(str::to_owned));
                if verbose {
                    print!("[{}/{total}] {theme}: fetching... ", i + 1);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                let outcome = match catalog::update_from_git(&ProcessExecutor, path, rev.as_deref())
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        failed += 1;
                        if verbose {
                            println!("failed: {error}");
                        } else if !quiet {
                            eprintln!("riso: {theme}: {error}");
                        }
                        report.push(serde_json::json!({ "theme": theme, "result": "failed" }));
                        continue;
                    }
                };
                let result = match outcome {
                    catalog::Updated::NotAClone => {
                        skipped += 1;
                        if verbose {
                            println!("not a git clone, skipped");
                        }
                        serde_json::json!({ "theme": theme, "result": "no git history" })
                    }
                    catalog::Updated::Current => {
                        current += 1;
                        if verbose {
                            println!("already up to date");
                        }
                        serde_json::json!({ "theme": theme, "result": "up to date" })
                    }
                    catalog::Updated::Moved { from, to } => {
                        // What arrived passes the same gate an install does;
                        // an update that turns a theme into a program goes
                        // back to the revision that was trusted.
                        if verbose {
                            print!("validating {}... ", short(&to));
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        }
                        let findings = riso_core::validate::validate(path, &Default::default())
                            .map_err(|e| e.to_string())?;
                        let fatal = findings.iter().filter(|f| f.is_fatal()).count();
                        if !quiet {
                            for finding in findings.iter().filter(|f| f.is_fatal()) {
                                if verbose {
                                    println!();
                                }
                                eprintln!("riso: REFUSE {theme}: {}", finding.describe());
                            }
                        }
                        if fatal > 0 && !trust {
                            failed += 1;
                            catalog::rollback(&ProcessExecutor, path, &from)
                                .map_err(|e| e.to_string())?;
                            if verbose {
                                println!("refused ({fatal} finding(s)), kept {}", short(&from));
                            } else if !quiet {
                                eprintln!(
                                    "riso: {theme}: update refused ({fatal} finding(s)), kept {}",
                                    short(&from)
                                );
                            }
                            serde_json::json!({ "theme": theme, "result": "refused" })
                        } else {
                            updated += 1;
                            if verbose {
                                println!("updated {} -> {}", short(&from), short(&to));
                            }
                            serde_json::json!({
                                "theme": theme,
                                "result": "updated",
                                "from": from,
                                "to": to,
                            })
                        }
                    }
                };
                report.push(result);
            }

            if verbose {
                println!(
                    "{updated} updated, {current} already current, {skipped} skipped, {failed} failed"
                );
            }
            if !quiet && output != OutputFormat::Human {
                emit(output, &report)?;
            }
            if failed > 0 {
                if quiet {
                    std::process::exit(1);
                }
                return Err(format!("{failed} theme update(s) did not go through"));
            }
            Ok(())
        }
        ThemeAction::Validate { path, warn_only } => {
            let findings = riso_core::validate::validate(&path, &Default::default())
                .map_err(|e| e.to_string())?;

            let fatal = findings.iter().filter(|f| f.is_fatal()).count();
            let report = serde_json::json!({
                "path": path,
                "fatal": fatal,
                "findings": findings
                    .iter()
                    .map(|f| serde_json::json!({
                        "severity": if f.is_fatal() { "refuse" } else { "warn" },
                        "message": f.describe(),
                    }))
                    .collect::<Vec<_>>(),
            });
            if !emit(output, &report)? {
                if findings.is_empty() {
                    println!("{}: clean", path.display());
                }
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
            }
            if findings.is_empty() {
                return Ok(());
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
            if !emit(output, &serde_json::json!({ "removed": path }))? {
                println!("removed {}", path.display());
            }
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
/// On an Omarchy system the desktop reads its state from its own directory,
/// so riso renders where that desktop looks and is a drop-in; anywhere else
/// the state is riso's own. `--state` overrides either way.
pub(crate) fn default_state_dir() -> Result<PathBuf, String> {
    let name = if std::env::var_os("OMARCHY_PATH").is_some_and(|v| !v.is_empty()) {
        "omarchy"
    } else {
        "riso"
    };
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join(name));
        }
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_owned())?;
    Ok(PathBuf::from(home).join(".local/state").join(name))
}

/// The format config.toml asks for, when `-o` does not override it. A value
/// the enum does not know is reported and ignored, never fatal.
fn default_output() -> OutputFormat {
    let config = riso_core::config::Config::load_or_default();
    match <OutputFormat as clap::ValueEnum>::from_str(&config.output, true) {
        Ok(format) => format,
        Err(_) => {
            eprintln!(
                "riso: config.toml asks for output {:?}, which is not human, json or yaml; using human",
                config.output
            );
            OutputFormat::Human
        }
    }
}

fn run_config(action: Option<ConfigAction>, output: OutputFormat) -> Result<(), String> {
    use riso_core::config::Config;

    match action.unwrap_or(ConfigAction::List) {
        ConfigAction::Check { state, desktop } => {
            let state_dir = state_or_default(state)?;
            let desktop = match desktop {
                Some(name) => Some(
                    Desktop::from_name(&name).ok_or_else(|| format!("unknown desktop '{name}'"))?,
                ),
                None => None,
            };
            let checks = check::run(
                &state_dir,
                &catalog::default_theme_dirs(),
                DEFAULT_CATALOG,
                desktop,
            );
            if !emit(output, &checks)? {
                check::print(&checks);
            }
            if checks.iter().any(|c| c.required && !c.ok) {
                return Err("a required tool is missing".to_owned());
            }
            Ok(())
        }
        ConfigAction::List => {
            let config = Config::load()?;
            if !emit(output, &config)? {
                println!("omarchy-themes = {}", config.omarchy_themes);
                println!("output = {}", config.output);
            }
            Ok(())
        }
        ConfigAction::Get { key } => {
            let config = Config::load()?;
            let value = match key.as_str() {
                "omarchy-themes" => config.omarchy_themes.to_string(),
                "output" => config.output,
                other => {
                    return Err(format!(
                        "unknown option {other}; `riso config` lists them all"
                    ))
                }
            };
            if !emit(output, &serde_json::json!({ key: value }))? {
                println!("{value}");
            }
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut config = Config::load()?;
            match key.as_str() {
                "omarchy-themes" => {
                    config.omarchy_themes = value.parse().map_err(|_| {
                        format!("omarchy-themes takes true or false, not {value:?}")
                    })?;
                }
                "output" => {
                    <OutputFormat as clap::ValueEnum>::from_str(&value, true)
                        .map_err(|_| format!("output takes human, json or yaml, not {value:?}"))?;
                    config.output = value.to_lowercase();
                }
                other => {
                    return Err(format!(
                        "unknown option {other}; `riso config` lists them all"
                    ))
                }
            }
            let path = config.save().map_err(|e| e.to_string())?;
            if !emit(output, &serde_json::json!({ "wrote": path }))? {
                println!("wrote {}", path.display());
            }
            Ok(())
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let output = cli.output.unwrap_or_else(default_output);
    match cli.command {
        Command::Theme { action } => run_theme(action, output),
        Command::Backgrounds { action } => run_bg(action, output),
        Command::Plugin { action } => run_plugin(action, output),
        Command::Dev { action } => run_dev(action, output),
        Command::Config { action } => run_config(action, output),
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
            let (restored, removed): (Vec<_>, Vec<_>) = done
                .into_iter()
                .partition(|o| matches!(o, riso_core::snapshot::Restored::Contents(_)));
            let as_paths = |v: Vec<riso_core::snapshot::Restored>| {
                v.into_iter()
                    .map(|o| match o {
                        riso_core::snapshot::Restored::Contents(p)
                        | riso_core::snapshot::Restored::Removed(p) => p,
                    })
                    .collect::<Vec<_>>()
            };
            let (restored, removed) = (as_paths(restored), as_paths(removed));
            if !emit(
                output,
                &serde_json::json!({ "restored": restored, "removed": removed }),
            )? {
                for path in &restored {
                    println!("restored {}", path.display());
                }
                for path in &removed {
                    println!("removed {}", path.display());
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

            let mut restored = Vec::new();
            let mut removed = Vec::new();
            for outcome in store.restore_all().map_err(|e| e.to_string())? {
                match outcome {
                    riso_core::snapshot::Restored::Contents(path) => restored.push(path),
                    riso_core::snapshot::Restored::Removed(path) => removed.push(path),
                }
            }
            std::fs::remove_dir_all(&state_dir)
                .map_err(|e| format!("removing {}: {e}", state_dir.display()))?;
            if !emit(
                output,
                &serde_json::json!({
                    "restored": restored,
                    "removed": removed,
                    "state_removed": state_dir,
                }),
            )? {
                for path in &restored {
                    println!("restored {}", path.display());
                }
                for path in &removed {
                    println!("removed {}", path.display());
                }
                println!("removed {}", state_dir.display());
            }
            Ok(())
        }
        Command::CarouselData { what, current } => run_carousel_data(&what, current),
    }
}

fn run_dev(action: DevAction, output: OutputFormat) -> Result<(), String> {
    match action {
        DevAction::Palette { theme } => {
            let (palette, warnings) = load_palette(&theme).map_err(|e| e.to_string())?;
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
        DevAction::Render {
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

            let written: Vec<_> = report
                .outcomes
                .iter()
                .filter_map(|o| match o {
                    Outcome::Rendered { target, .. } => Some(target.clone()),
                    Outcome::Kept { .. } => None,
                })
                .collect();
            let kept: Vec<_> = report
                .outcomes
                .iter()
                .filter_map(|o| match o {
                    Outcome::Kept { target, .. } => Some(target.clone()),
                    Outcome::Rendered { .. } => None,
                })
                .collect();
            let summary =
                serde_json::json!({ "dry_run": dry_run, "written": written, "kept": kept });
            if !emit(output, &summary)? {
                for target in &written {
                    println!(
                        "{} {}",
                        if dry_run { "would write" } else { "wrote" },
                        target.display()
                    );
                }
                for target in &kept {
                    println!("kept {} (provided by the theme)", target.display());
                }
            }
            Ok(())
        }
    }
}

fn state_or_default(state: Option<PathBuf>) -> Result<PathBuf, String> {
    match state {
        Some(dir) => Ok(dir),
        None => default_state_dir(),
    }
}

/// Point the current-background link at `image` and tell the desktop that
/// draws its own wallpaper.
/// The theme in use, as the state tree records it.
fn current_theme_name(state: &Path) -> Option<String> {
    let name = std::fs::read_to_string(state.join("current/theme.name")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn set_background(
    image: &Path,
    state: &Path,
    no_reload: bool,
    output: OutputFormat,
) -> Result<(), String> {
    let image = std::fs::canonicalize(image).map_err(|e| format!("{}: {e}", image.display()))?;
    if !image.is_file() {
        return Err(format!("{}: not a file", image.display()));
    }

    let link = state.join("current/background");
    riso_core::background::link(&link, &image).map_err(|e| e.to_string())?;

    // Record the pick against the theme showing it, so returning to that
    // theme returns to this image instead of starting its list over.
    if let Some(theme) = current_theme_name(state) {
        let _ = riso_core::background::remember(state, &theme, &image);
    }

    if !no_reload {
        let desktop = Desktop::detect();
        let shell_config = std::env::var_os("OMARCHY_PATH")
            .map(|p| PathBuf::from(p).join("shell"))
            .filter(|p| p.is_dir());
        desktop
            .set_background(&ProcessExecutor, &image, shell_config.as_deref())
            .map_err(|e| e.to_string())?;
    }

    if !emit(output, &serde_json::json!({ "background": image }))? {
        println!("background {}", image.display());
    }
    Ok(())
}

fn run_bg(action: BgAction, output: OutputFormat) -> Result<(), String> {
    let _ = output;
    match action {
        BgAction::Set {
            image,
            gui,
            tui,
            state,
            no_reload,
        } => {
            if gui {
                return gui::run(data::What::Backgrounds, data::Purpose::Apply);
            }
            if tui {
                return tui::run(data::What::Backgrounds, data::Purpose::Apply, state);
            }
            let Some(image) = image else {
                return Err("an image is needed, or --gui/--tui to pick one".to_owned());
            };
            let state = state_or_default(state)?;
            set_background(&image, &state, no_reload, output)
        }
        BgAction::Next { state, no_reload } => {
            let state = state_or_default(state)?;
            let images =
                riso_core::background::candidates(&[state.join("current/theme/backgrounds")]);
            let showing = riso_core::background::current(&state.join("current/background"));
            let Some(chosen) = riso_core::background::next(&images, showing.as_deref()) else {
                return Err("the current theme ships no backgrounds".to_owned());
            };
            set_background(&chosen, &state, no_reload, output)
        }
        BgAction::Mode { mode, state } => {
            let state = state_or_default(state)?;
            let path = state.join("current/background.mode");
            match mode {
                Some(mode) => {
                    riso_core::atomic::write_atomic(&path, &format!("{mode}\n"))
                        .map_err(|e| e.to_string())?;
                    if !emit(output, &serde_json::json!({ "mode": mode }))? {
                        println!("mode {mode}");
                    }
                }
                None => {
                    let mode = std::fs::read_to_string(&path).unwrap_or_default();
                    let mode = mode.trim();
                    let mode = if mode.is_empty() { "fill" } else { mode };
                    if !emit(output, &serde_json::json!({ "mode": mode }))? {
                        println!("{mode}");
                    }
                }
            }
            Ok(())
        }
        BgAction::Get { state } => {
            let state = state_or_default(state)?;
            let image = riso_core::background::current(&state.join("current/background"));
            let mode =
                std::fs::read_to_string(state.join("current/background.mode")).unwrap_or_default();
            let mode = mode.trim();
            let mode = if mode.is_empty() { "fill" } else { mode };
            if !emit(output, &serde_json::json!({ "image": image, "mode": mode }))? {
                match &image {
                    Some(path) => println!("{}", path.display()),
                    None => println!("none"),
                }
                println!("mode {mode}");
            }
            Ok(())
        }
    }
}

fn run_carousel_data(what: &str, current: bool) -> Result<(), String> {
    let state = default_state_dir()?;

    if current {
        match what {
            "backgrounds" => {
                if let Some(path) = data::current_background(&state) {
                    println!("{}", path.display());
                }
            }
            _ => {
                if let Some(name) = data::current_theme(&state) {
                    println!("{name}");
                }
            }
        }
        return Ok(());
    }

    let rows = match what {
        "backgrounds" => data::background_rows(&state),
        "catalog" => data::catalog_rows(&ProcessExecutor, DEFAULT_CATALOG),
        _ => data::theme_rows(),
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
