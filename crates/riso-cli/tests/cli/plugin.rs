//! Plugins over their whole life: install from git, render on theme
//! set, listing, and removal.

use crate::common::{err, out, Sandbox};
use std::path::PathBuf;

fn git_plugin_repo(sb: &Sandbox) -> PathBuf {
    let repo = sb.home().join("repos/zed-plugin");
    std::fs::create_dir_all(&repo).expect("mkdir");
    std::fs::write(
        repo.join("manifest.toml"),
        "id = \"zed\"\napi = 1\n\n[[render]]\ntemplate = \"zed.json.tpl\"\ntarget = \"~/.config/zed/riso.json\"\n",
    )
    .expect("write");
    std::fs::write(
        repo.join("zed.json.tpl"),
        "{\"accent\": \"{{ accent }}\"}\n",
    )
    .expect("write");
    sb.git(&repo, &["init", "-q", "-b", "main"]);
    sb.git(&repo, &["add", "."]);
    sb.git(&repo, &["commit", "-q", "-m", "plugin"]);
    repo
}

#[test]
fn a_plugin_installs_lists_renders_and_removes() {
    let sb = Sandbox::new();
    sb.real_bin("git");
    let repo = git_plugin_repo(&sb);

    let run = sb.riso(&["plugin", "install", &format!("file://{}", repo.display())]);
    assert!(run.status.success(), "stderr: {}", err(&run));

    let run = sb.riso(&["plugin", "list"]);
    assert!(out(&run).contains("zed"), "stdout: {}", out(&run));

    sb.theme("tokyo-night");
    let run = sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let rendered =
        std::fs::read_to_string(sb.home().join(".config/zed/riso.json")).expect("rendered");
    assert!(rendered.contains("#7aa2f7"));

    let run = sb.riso(&["config", "apps"]);
    assert!(out(&run).contains("plugin zed"), "stdout: {}", out(&run));

    let run = sb.riso(&["plugin", "remove", "zed"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let run = sb.riso(&["plugin", "list"]);
    assert!(!out(&run).contains("zed"));
}
