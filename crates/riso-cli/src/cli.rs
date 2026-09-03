//! The command line as clap sees it: every subcommand, flag and alias.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::output::OutputFormat;
use crate::paths::DEFAULT_CATALOG;

#[derive(Parser)]
#[command(name = "riso", version, about = "Modular ricing framework")]
pub(crate) struct Cli {
    /// Output format for results; defaults to `output` in config.toml
    #[arg(short = 'o', long = "output", global = true, value_enum)]
    pub(crate) output: Option<OutputFormat>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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
pub(crate) enum ConfigAction {
    /// Print every option and its value
    #[command(visible_alias = "l")]
    List,
    /// What the current configuration can theme, and where each piece
    /// comes from: built-in, theme, or plugin
    #[command(visible_alias = "a")]
    Apps {
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
        /// Template directory; repeat for more, earlier ones take precedence
        #[arg(long = "templates", value_name = "DIR")]
        template_dirs: Vec<PathBuf>,
    },
    /// Add the include lines config check reports missing, cautiously:
    /// plan shown first, one confirmation per file, riso restore undoes
    #[command(visible_alias = "w")]
    Wire {
        /// Applications to wire; omit to plan every installed one
        apps: Vec<String>,
        /// Apply without asking, for scripts
        #[arg(long)]
        yes: bool,
        /// Where the generated theme lives
        #[arg(long, value_name = "DIR")]
        state: Option<PathBuf>,
    },
    /// Can this system carry riso: tools, desktop, and include wiring
    #[command(visible_alias = "k")]
    Check {
        /// One check by name: a tool, a section, or an application
        /// (shown even when not installed); omit it for the whole system
        name: Option<String>,
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
pub(crate) enum BgAction {
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
pub(crate) enum ThemeAction {
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
pub(crate) enum PluginAction {
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
pub(crate) enum DevAction {
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
