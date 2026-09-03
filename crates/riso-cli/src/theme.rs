//! `riso theme`: apply, list, install, update, validate, remove.

use std::path::{Path, PathBuf};

use riso_core::apply::{apply, Parts, Request};
use riso_core::catalog;
use riso_core::desktop::Desktop;
use riso_core::reload::ProcessExecutor;
use riso_core::validate;

use crate::cli::ThemeAction;
use crate::output::{emit, report_warnings, OutputFormat};
use crate::paths::{
    home_dir, omarchy_path, omarchy_shell_config, state_or_default, user_plugin_dir, user_theme_dir,
};
use crate::{data, gui, tui};

pub(crate) fn run(action: ThemeAction, output: OutputFormat) -> Result<(), String> {
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
            if let Some(picked) = pick(gui, tui, data::What::Themes, data::Purpose::Apply, &state) {
                return picked;
            }
            let Some(name) = name else {
                return Err("a theme name is needed, or --gui/--tui to pick one".to_owned());
            };
            let options = SetOptions {
                theme_dirs,
                template_dirs,
                state,
                no_reload,
                no_builtin,
                desktop,
                plugin_dirs,
            };
            set(&build_request(name, options)?, output)
        }
        ThemeAction::Get { state } => get(state, output),
        ThemeAction::List {
            theme_dirs,
            gui,
            tui,
        } => {
            if let Some(picked) = pick(gui, tui, data::What::Themes, data::Purpose::Browse, &None) {
                return picked;
            }
            list(theme_dirs, output)
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
            if let Some(picked) = pick(gui, tui, data::What::Catalog, data::Purpose::Install, &None)
            {
                return picked;
            }
            let Some(source) = source else {
                return Err(
                    "a theme name or git URL is needed, or --gui/--tui to browse".to_owned(),
                );
            };
            install(&source, into, &index_url, name, trust, output)
        }
        ThemeAction::Update {
            name,
            into,
            catalog: index_url,
            trust,
            quiet,
        } => update(name, into, &index_url, trust, quiet, output),
        ThemeAction::Validate { path, warn_only } => run_validate(&path, warn_only, output),
        ThemeAction::Remove { name, into } => remove(&name, into, output),
    }
}

/// The picker, when a flag asked for one instead of a name.
fn pick(
    gui: bool,
    tui: bool,
    what: data::What,
    purpose: data::Purpose,
    state: &Option<PathBuf>,
) -> Option<Result<(), String>> {
    if gui {
        return Some(gui::run(what, purpose));
    }
    if tui {
        return Some(tui::run(what, purpose, state.clone()));
    }
    None
}

/// Restore the built-in theme, reporting rather than failing: it is a
/// way out of having nothing, so refusing to continue would defeat it.
fn seed_fallback(dir: &Path) {
    match catalog::seed_fallback(dir) {
        Ok(path) => eprintln!(
            "riso: no themes found, wrote the built-in one to {}",
            path.display()
        ),
        Err(e) => eprintln!("riso: could not write the built-in theme: {e}"),
    }
}

fn resolve_desktop(named: Option<String>) -> Result<Desktop, String> {
    match named {
        Some(name) => Desktop::from_name(&name).ok_or_else(|| format!("unknown desktop '{name}'")),
        None => Ok(Desktop::detect()),
    }
}

/// The search path for a set, seeding the built-in theme when the
/// machine has none at all: it is written out as an ordinary theme
/// rather than special-cased.
fn theme_dirs_for_set(theme_dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = if theme_dirs.is_empty() {
        catalog::default_theme_dirs()
    } else {
        theme_dirs
    };
    if catalog::installed(&dirs, None).is_empty() {
        if let Some(user) = catalog::user_theme_dir() {
            seed_fallback(&user);
            if !dirs.contains(&user) {
                dirs.push(user);
            }
        }
    }
    dirs
}

/// On an Omarchy system its template layers apply by default: the
/// user's own, then the desktop's. Explicit flags replace them entirely.
fn template_dirs_for_set(template_dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    if !template_dirs.is_empty() {
        return template_dirs;
    }
    let Some(omarchy) = omarchy_path() else {
        return template_dirs;
    };
    let mut dirs = Vec::new();
    if let Ok(home) = home_dir() {
        dirs.push(home.join(".config/omarchy/themed"));
    }
    dirs.push(omarchy.join("default/themed"));
    dirs
}

/// The flags of `theme set`, as the command line hands them over.
struct SetOptions {
    theme_dirs: Vec<PathBuf>,
    template_dirs: Vec<PathBuf>,
    state: Option<PathBuf>,
    no_reload: bool,
    no_builtin: bool,
    desktop: Option<String>,
    plugin_dirs: Vec<PathBuf>,
}

fn build_request(name: String, options: SetOptions) -> Result<Request, String> {
    Ok(Request {
        name,
        theme_dirs: theme_dirs_for_set(options.theme_dirs),
        template_dirs: template_dirs_for_set(options.template_dirs),
        state_dir: state_or_default(options.state)?,
        background_dirs: Vec::new(),
        hooks: Vec::new(),
        parts: Parts::default(),
        builtin_templates: !options.no_builtin,
        plugin_dirs: if options.plugin_dirs.is_empty() {
            vec![user_plugin_dir()?]
        } else {
            options.plugin_dirs
        },
        home: home_dir()?,
        desktop: resolve_desktop(options.desktop)?,
        shell_config: omarchy_shell_config(),
        skip_reload: options.no_reload,
    })
}

fn set(request: &Request, output: OutputFormat) -> Result<(), String> {
    let applied = apply(request, &ProcessExecutor).map_err(|e| e.to_string())?;
    report_warnings(&applied.warnings);
    let desktop = request.desktop.name();
    let summary = serde_json::json!({
        "theme": applied.name,
        "target": applied.target,
        "rendered": applied.report.rendered().count(),
        "kept": applied.report.kept().count(),
        "desktop": desktop,
        "background": applied.background,
    });
    if !emit(output, &summary)? {
        println!(
            "applied {} to {} ({} rendered, {} from the theme, desktop: {desktop})",
            applied.name,
            applied.target.display(),
            applied.report.rendered().count(),
            applied.report.kept().count(),
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

fn get(state: Option<PathBuf>, output: OutputFormat) -> Result<(), String> {
    let state = state_or_default(state)?;
    let Some(name) = data::current_theme(&state) else {
        return Err("no theme is applied".to_owned());
    };
    if !emit(output, &serde_json::json!({ "name": name }))? {
        println!("{name}");
    }
    Ok(())
}

fn list(theme_dirs: Vec<PathBuf>, output: OutputFormat) -> Result<(), String> {
    let user = user_theme_dir()?;
    let mut dirs = if theme_dirs.is_empty() {
        // installed() lets the first occurrence win, the reverse of the
        // overlay order the search path is written in.
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
            let tag = if row["read_only"] == true {
                "\t(read-only)"
            } else {
                ""
            };
            println!(
                "{}\t{}{tag}",
                row["name"].as_str().unwrap_or_default(),
                row["path"].as_str().unwrap_or_default(),
            );
        }
    }
    Ok(())
}

/// A git URL is taken as is; anything else is looked up in the catalog.
fn resolve_source(source: &str, index_url: &str) -> Result<(String, Option<String>), String> {
    if source.contains("://") || source.contains('@') {
        return Ok((source.to_owned(), None));
    }
    let index = catalog::fetch_index(&ProcessExecutor, index_url).map_err(|e| e.to_string())?;
    let entry = index
        .find(source)
        .ok_or_else(|| format!("no theme named '{source}' in the catalog"))?;
    if let Some(reason) = &entry.yanked {
        return Err(format!(
            "'{source}' was withdrawn from the catalog: {reason}"
        ));
    }
    Ok((entry.repo.clone(), entry.rev.clone()))
}

/// Fatal findings, each reported, in a theme that just arrived.
fn fatal_findings(path: &Path, name: &str, quiet: bool) -> Result<usize, String> {
    let findings = validate::validate(path, &Default::default()).map_err(|e| e.to_string())?;
    let fatal: Vec<_> = findings.iter().filter(|f| f.is_fatal()).collect();
    if !quiet {
        for finding in &fatal {
            eprintln!("riso: REFUSE {name}: {}", finding.describe());
        }
    }
    Ok(fatal.len())
}

fn install(
    source: &str,
    into: Option<PathBuf>,
    index_url: &str,
    name: Option<String>,
    trust: bool,
    output: OutputFormat,
) -> Result<(), String> {
    let into = match into {
        Some(dir) => dir,
        None => user_theme_dir()?,
    };
    let (repo, rev) = resolve_source(source, index_url)?;
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

    // The same gate the catalog runs, because this theme may never have
    // passed through one: a clone straight from a git URL is exactly the
    // case the client-side check exists for. Only the findings that make
    // a theme unsafe matter here; catalog policy (license, size) belongs
    // to `theme validate` and the CI that runs it.
    let fatal = fatal_findings(&path, &name, false)?;
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

/// How one `update` run talks: -q says nothing and lets the exit code
/// speak, -o json/yaml keeps the machine report, plain human mode
/// narrates each theme as it happens.
struct Voice {
    quiet: bool,
    verbose: bool,
}

impl Voice {
    fn say(&self, text: &str) {
        if self.verbose {
            println!("{text}");
        }
    }

    fn progress(&self, text: &str) {
        if self.verbose {
            print!("{text}");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    }

    /// A problem, on the narrated line when narrating, on stderr otherwise.
    fn problem(&self, theme: &str, narrated: &str, plain: &str) {
        if self.verbose {
            println!("{narrated}");
        } else if !self.quiet {
            eprintln!("riso: {theme}: {plain}");
        }
    }
}

struct Tally {
    updated: usize,
    current: usize,
    skipped: usize,
    failed: usize,
    report: Vec<serde_json::Value>,
}

fn short(rev: &str) -> &str {
    &rev[..rev.len().min(7)]
}

/// The themes an update covers: one by name, or every clone installed.
fn update_targets(name: Option<&str>, into: &Path) -> Result<Vec<PathBuf>, String> {
    match name {
        Some(name) => {
            let path = into.join(name);
            if !path.is_dir() {
                return Err(format!("nothing named '{name}' is installed"));
            }
            Ok(vec![path])
        }
        None => Ok(catalog::installed(&[into.to_path_buf()], Some(into))
            .into_iter()
            .map(|theme| theme.path)
            .collect()),
    }
}

/// Fetch one theme and, when it moved, hold it to the same gate an
/// install passes: an update that turns a theme into a program goes back
/// to the revision that was trusted.
fn update_one(
    path: &Path,
    theme: &str,
    rev: Option<&str>,
    trust: bool,
    voice: &Voice,
    tally: &mut Tally,
) -> Result<(), String> {
    let outcome = match catalog::update_from_git(&ProcessExecutor, path, rev) {
        Ok(outcome) => outcome,
        Err(error) => {
            tally.failed += 1;
            voice.problem(theme, &format!("failed: {error}"), &error.to_string());
            tally
                .report
                .push(serde_json::json!({ "theme": theme, "result": "failed" }));
            return Ok(());
        }
    };
    let result = match outcome {
        catalog::Updated::NotAClone => {
            tally.skipped += 1;
            voice.say("not a git clone, skipped");
            serde_json::json!({ "theme": theme, "result": "no git history" })
        }
        catalog::Updated::Current => {
            tally.current += 1;
            voice.say("already up to date");
            serde_json::json!({ "theme": theme, "result": "up to date" })
        }
        catalog::Updated::Moved { from, to } => {
            voice.progress(&format!("validating {}... ", short(&to)));
            let fatal = fatal_findings(path, theme, voice.quiet)?;
            if fatal > 0 && !trust {
                tally.failed += 1;
                catalog::rollback(&ProcessExecutor, path, &from).map_err(|e| e.to_string())?;
                voice.problem(
                    theme,
                    &format!("refused ({fatal} finding(s)), kept {}", short(&from)),
                    &format!("update refused ({fatal} finding(s)), kept {}", short(&from)),
                );
                serde_json::json!({ "theme": theme, "result": "refused" })
            } else {
                tally.updated += 1;
                voice.say(&format!("updated {} -> {}", short(&from), short(&to)));
                serde_json::json!({ "theme": theme, "result": "updated", "from": from, "to": to })
            }
        }
    };
    tally.report.push(result);
    Ok(())
}

fn update(
    name: Option<String>,
    into: Option<PathBuf>,
    index_url: &str,
    trust: bool,
    quiet: bool,
    output: OutputFormat,
) -> Result<(), String> {
    let into = match into {
        Some(dir) => dir,
        None => user_theme_dir()?,
    };
    // The catalog is a hint, not a gate: a theme it does not carry updates
    // from its own origin. Failing to fetch it only means no revisions are
    // pinned.
    let index = catalog::fetch_index(&ProcessExecutor, index_url).ok();
    let targets = update_targets(name.as_deref(), &into)?;
    let voice = Voice {
        quiet,
        verbose: !quiet && output == OutputFormat::Human,
    };
    let total = targets.len();
    voice.say(&format!(
        "updating {total} theme{} in {}",
        if total == 1 { "" } else { "s" },
        into.display()
    ));

    let mut tally = Tally {
        updated: 0,
        current: 0,
        skipped: 0,
        failed: 0,
        report: Vec::new(),
    };
    for (i, path) in targets.iter().enumerate() {
        let theme = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rev = index
            .as_ref()
            .and_then(|i| i.find(&theme))
            .and_then(|e| e.rev.clone());
        voice.progress(&format!("[{}/{total}] {theme}: fetching... ", i + 1));
        update_one(path, &theme, rev.as_deref(), trust, &voice, &mut tally)?;
    }

    voice.say(&format!(
        "{} updated, {} already current, {} skipped, {} failed",
        tally.updated, tally.current, tally.skipped, tally.failed
    ));
    if !quiet && output != OutputFormat::Human {
        emit(output, &tally.report)?;
    }
    if tally.failed > 0 {
        if quiet {
            std::process::exit(1);
        }
        return Err(format!(
            "{} theme update(s) did not go through",
            tally.failed
        ));
    }
    Ok(())
}

fn run_validate(path: &Path, warn_only: bool, output: OutputFormat) -> Result<(), String> {
    let findings = validate::validate(path, &Default::default()).map_err(|e| e.to_string())?;
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
            let tag = if finding.is_fatal() {
                "REFUSE"
            } else {
                "warn  "
            };
            println!("{tag} {}", finding.describe());
        }
    }
    if fatal > 0 && !warn_only {
        return Err(format!(
            "{fatal} finding(s) make this theme unsafe to install"
        ));
    }
    Ok(())
}

fn remove(name: &str, into: Option<PathBuf>, output: OutputFormat) -> Result<(), String> {
    let into = match into {
        Some(dir) => dir,
        None => user_theme_dir()?,
    };
    let path =
        catalog::remove(name, std::slice::from_ref(&into), &into).map_err(|e| e.to_string())?;
    if !emit(output, &serde_json::json!({ "removed": path }))? {
        println!("removed {}", path.display());
    }
    Ok(())
}
