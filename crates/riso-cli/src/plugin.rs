//! `riso plugin`: list, install from git, remove.

use riso_core::catalog;
use riso_core::reload::ProcessExecutor;

use crate::cli::PluginAction;
use crate::output::{emit, OutputFormat};
use crate::paths::user_plugin_dir;

pub(crate) fn run(action: PluginAction, output: OutputFormat) -> Result<(), String> {
    match action {
        PluginAction::List { plugin_dirs } => list(plugin_dirs, output),
        PluginAction::Install { repo, into, name } => install(repo, into, name, output),
        PluginAction::Remove { name, into } => remove(name, into, output),
    }
}

fn list(plugin_dirs: Vec<std::path::PathBuf>, output: OutputFormat) -> Result<(), String> {
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

fn install(
    repo: String,
    into: Option<std::path::PathBuf>,
    name: Option<String>,
    output: OutputFormat,
) -> Result<(), String> {
    let into = match into {
        Some(dir) => dir,
        None => user_plugin_dir()?,
    };
    let name = name.unwrap_or_else(|| catalog::name_from_repo(&repo));
    if !catalog::is_safe_name(&name) {
        return Err(format!("'{name}' is not usable as a directory name"));
    }
    // A plugin is code: cloning it is the moment to say so.
    eprintln!("riso: a plugin runs as code on your machine; review {repo} before enabling it");
    let path =
        catalog::install_from_git(&ProcessExecutor, &repo, None, &name, &into, "manifest.toml")
            .map_err(|e| e.to_string())?;
    if !emit(
        output,
        &serde_json::json!({ "installed": name, "path": path }),
    )? {
        println!("installed {name} to {}", path.display());
    }
    Ok(())
}

fn remove(
    name: String,
    into: Option<std::path::PathBuf>,
    output: OutputFormat,
) -> Result<(), String> {
    let into = match into {
        Some(dir) => dir,
        None => user_plugin_dir()?,
    };
    // The name may be the directory or the manifest id `list` prints;
    // both must work.
    let mut path = into.join(&name);
    if !path.join("manifest.toml").is_file() {
        let plugins =
            riso_core::plugin::discover(std::slice::from_ref(&into)).map_err(|e| e.to_string())?;
        match plugins.into_iter().find(|p| p.manifest.id == name) {
            Some(plugin) => path = plugin.dir,
            None => return Err(format!("nothing named '{name}' is installed")),
        }
    }
    std::fs::remove_dir_all(&path).map_err(|e| format!("removing {}: {e}", path.display()))?;
    if !emit(output, &serde_json::json!({ "removed": path }))? {
        println!("removed {}", path.display());
    }
    Ok(())
}
