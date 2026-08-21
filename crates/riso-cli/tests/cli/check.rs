//! `riso config check`: single checks by name, the lua Hyprland hint,
//! and the exit code carrying the verdict.

use crate::common::{err, out, Sandbox};

#[test]
fn a_tool_on_path_passes_and_one_missing_fails() {
    let sb = Sandbox::new();
    sb.fake_bin("git");
    let run = sb.riso(&["config", "check", "git"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("git"));

    let run = sb.riso(&["config", "check", "curl"]);
    assert!(!run.status.success());
}

#[test]
fn an_unknown_name_is_an_error_that_lists_the_choices() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "check", "nope"]);
    assert!(!run.status.success());
    assert!(err(&run).contains("nope"));
}

#[test]
fn an_unwired_lua_hyprland_hint_is_two_short_lines() {
    let sb = Sandbox::new();
    sb.fake_bin("Hyprland");
    sb.write(".config/hypr/hyprland.lua", "-- mine\n");
    let run = sb.riso(&["config", "check", "hyprland"]);
    assert!(!run.status.success());
    let text = out(&run);
    let lua: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("local ok") || l.starts_with("if err"))
        .collect();
    assert_eq!(lua.len(), 2, "hint should be two lua lines:\n{text}");
    assert!(lua[0].contains("pcall(dofile,"));
    assert_eq!(lua[1], "if err then print(err) end");
}

#[test]
fn a_wired_lua_hyprland_config_passes() {
    let sb = Sandbox::new();
    sb.fake_bin("Hyprland");
    sb.write(
        ".config/hypr/hyprland.lua",
        "dofile(\"/x/riso/current/theme/hyprland.lua\")\n",
    );
    let run = sb.riso(&["config", "check", "hyprland"]);
    assert!(run.status.success(), "stdout: {}", out(&run));
}

#[test]
fn a_hyprlang_hyprland_config_still_gets_the_source_hint() {
    let sb = Sandbox::new();
    sb.fake_bin("Hyprland");
    sb.write(".config/hypr/hyprland.conf", "# mine\n");
    let run = sb.riso(&["config", "check", "hyprland"]);
    assert!(!run.status.success());
    assert!(out(&run).contains("source = "));
}

#[test]
fn the_omarchy_desktop_hands_wiring_to_the_desktop() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "check", "--desktop", "omarchy"]);
    assert!(out(&run).contains("omarchy"), "stdout: {}", out(&run));
}
