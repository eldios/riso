//! `riso config`: the few options in config.toml, and the diagnostics
//! that live under the same command (apps, check, wire).

use std::path::PathBuf;

use riso_core::catalog;
use riso_core::config::Config;
use riso_core::desktop::Desktop;
use riso_core::snapshot::Store;

use crate::cli::ConfigAction;
use crate::output::{emit, OutputFormat};
use crate::paths::{state_or_default, user_plugin_dir, DEFAULT_CATALOG};
use crate::{apps, check, wire};

pub(crate) fn run(action: Option<ConfigAction>, output: OutputFormat) -> Result<(), String> {
    match action.unwrap_or(ConfigAction::List) {
        ConfigAction::Wire { apps, yes, state } => run_wire(apps, yes, state, output),
        ConfigAction::Apps {
            state,
            template_dirs,
        } => run_apps(state, template_dirs, output),
        ConfigAction::Check {
            name,
            state,
            desktop,
        } => run_check(name, state, desktop, output),
        ConfigAction::List => list(output),
        ConfigAction::Get { key } => get(&key, output),
        ConfigAction::Set { key, value } => set(&key, value, output),
    }
}

fn run_wire(
    apps: Vec<String>,
    yes: bool,
    state: Option<PathBuf>,
    output: OutputFormat,
) -> Result<(), String> {
    let state_dir = state_or_default(state)?;
    let plans = wire::plan(&state_dir, &apps, wire::declarative_system())?;
    if emit(output, &plans)? && !yes {
        return Ok(());
    }
    let human = output == OutputFormat::Human;
    let mut store: Option<Store> = None;
    let (mut wired, mut skipped) = (0, 0);
    for plan in &plans {
        if human {
            describe(plan);
        }
        let go = wire::actionable(plan)
            && (yes || (human && wire::confirm(&format!("  wire {}?", plan.app))));
        if !go {
            skipped += 1;
            continue;
        }
        if store.is_none() {
            store = Some(Store::open(&state_dir.join("ownership")).map_err(|e| e.to_string())?);
        }
        wire::apply(plan, store.as_mut().expect("opened above"))?;
        wired += 1;
        if human {
            println!("  wired.");
        }
    }
    if human {
        println!("{wired} wired, {skipped} left as they were; riso restore undoes it");
    }
    Ok(())
}

/// One plan on screen, with the line to place by hand where riso will
/// not place it.
fn describe(plan: &wire::Plan) {
    println!("{}", wire::describe(plan));
    if matches!(
        plan.action,
        wire::Action::Manual | wire::Action::Declarative
    ) {
        for line in plan.text.lines() {
            println!("    {line}");
        }
    }
}

fn run_apps(
    state: Option<PathBuf>,
    template_dirs: Vec<PathBuf>,
    output: OutputFormat,
) -> Result<(), String> {
    let state_dir = state_or_default(state)?;
    let rows = apps::run(
        &state_dir,
        &catalog::default_theme_dirs(),
        &template_dirs,
        &[user_plugin_dir()?],
    )?;
    if !emit(output, &rows)? {
        apps::print(&rows);
    }
    Ok(())
}

fn run_check(
    name: Option<String>,
    state: Option<PathBuf>,
    desktop: Option<String>,
    output: OutputFormat,
) -> Result<(), String> {
    let state_dir = state_or_default(state)?;
    let desktop = match desktop {
        Some(named) => {
            Some(Desktop::from_name(&named).ok_or_else(|| format!("unknown desktop '{named}'"))?)
        }
        None => None,
    };
    let theme_dirs = catalog::default_theme_dirs();
    let Some(name) = name else {
        let checks = check::run(&state_dir, &theme_dirs, DEFAULT_CATALOG, desktop);
        if !emit(output, &checks)? {
            check::print(&checks);
        }
        if checks.iter().any(|c| c.required && !c.ok) {
            return Err("a required tool is missing".to_owned());
        }
        return Ok(());
    };
    let checks = check::select(&name, &state_dir, &theme_dirs, DEFAULT_CATALOG, desktop)?;
    if !emit(output, &checks)? {
        check::print(&checks);
    }
    // A single check answers with its exit code alone.
    if checks.iter().any(|c| !c.ok) {
        std::process::exit(1);
    }
    Ok(())
}

fn list(output: OutputFormat) -> Result<(), String> {
    let config = Config::load()?;
    if !emit(output, &config)? {
        println!("omarchy-themes = {}", config.omarchy_themes);
        println!("output = {}", config.output);
    }
    Ok(())
}

fn get(key: &str, output: OutputFormat) -> Result<(), String> {
    let config = Config::load()?;
    let value = match key {
        "omarchy-themes" => config.omarchy_themes.to_string(),
        "output" => config.output,
        other => return Err(unknown(other)),
    };
    if !emit(output, &serde_json::json!({ key: value }))? {
        println!("{value}");
    }
    Ok(())
}

fn set(key: &str, value: String, output: OutputFormat) -> Result<(), String> {
    let mut config = Config::load()?;
    match key {
        "omarchy-themes" => {
            config.omarchy_themes = value
                .parse()
                .map_err(|_| format!("omarchy-themes takes true or false, not {value:?}"))?;
        }
        "output" => {
            <OutputFormat as clap::ValueEnum>::from_str(&value, true)
                .map_err(|_| format!("output takes human, json or yaml, not {value:?}"))?;
            config.output = value.to_lowercase();
        }
        other => return Err(unknown(other)),
    }
    let path = config.save().map_err(|e| e.to_string())?;
    if !emit(output, &serde_json::json!({ "wrote": path }))? {
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn unknown(key: &str) -> String {
    format!("unknown option {key}; `riso config` lists them all")
}
