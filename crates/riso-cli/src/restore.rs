//! `riso restore` and `riso uninstall`: the ownership store handing
//! every file back.

use std::path::PathBuf;

use riso_core::snapshot::{Restored, Store};

use crate::output::{emit, OutputFormat};
use crate::paths::state_or_default;

pub(crate) fn restore(
    state: Option<PathBuf>,
    path: Option<PathBuf>,
    output: OutputFormat,
) -> Result<(), String> {
    let state_dir = state_or_default(state)?;
    let mut store = open(&state_dir)?;
    let done: Vec<Restored> = match path {
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
    let (restored, removed) = split(done);
    if !emit(
        output,
        &serde_json::json!({ "restored": restored, "removed": removed }),
    )? {
        print_paths(&restored, &removed);
    }
    Ok(())
}

pub(crate) fn uninstall(
    state: Option<PathBuf>,
    yes: bool,
    output: OutputFormat,
) -> Result<(), String> {
    let state_dir = state_or_default(state)?;
    let mut store = open(&state_dir)?;
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
    let (restored, removed) = split(store.restore_all().map_err(|e| e.to_string())?);
    std::fs::remove_dir_all(&state_dir)
        .map_err(|e| format!("removing {}: {e}", state_dir.display()))?;
    let summary = serde_json::json!({
        "restored": restored,
        "removed": removed,
        "state_removed": state_dir,
    });
    if !emit(output, &summary)? {
        print_paths(&restored, &removed);
        println!("removed {}", state_dir.display());
    }
    Ok(())
}

fn open(state_dir: &std::path::Path) -> Result<Store, String> {
    Store::open(&state_dir.join("ownership")).map_err(|e| e.to_string())
}

/// Outcomes as two path lists: contents put back, files removed.
fn split(done: Vec<Restored>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut restored = Vec::new();
    let mut removed = Vec::new();
    for outcome in done {
        match outcome {
            Restored::Contents(path) => restored.push(path),
            Restored::Removed(path) => removed.push(path),
        }
    }
    (restored, removed)
}

fn print_paths(restored: &[PathBuf], removed: &[PathBuf]) {
    for path in restored {
        println!("restored {}", path.display());
    }
    for path in removed {
        println!("removed {}", path.display());
    }
}
