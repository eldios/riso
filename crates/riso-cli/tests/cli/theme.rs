//! Themes through the real binary: listing, the plain fallback's
//! visibility, loose name resolution, and what a set leaves in the
//! state tree.

use crate::common::{err, out, Sandbox};

#[test]
fn with_no_themes_installed_only_plain_shows() {
    let sb = Sandbox::new();
    let run = sb.riso(&["theme", "list"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("plain"), "stdout: {}", out(&run));
}

#[test]
fn any_installed_theme_hides_the_plain_fallback() {
    let sb = Sandbox::new();
    sb.theme("tokyo-night");
    let run = sb.riso(&["theme", "list"]);
    assert!(run.status.success());
    let text = out(&run);
    assert!(text.contains("tokyo-night"));
    assert!(!text.contains("plain"), "stdout: {text}");
}

#[test]
fn set_resolves_a_loosely_spelled_name() {
    let sb = Sandbox::new();
    sb.theme("cyber-punk-red");
    let run = sb.riso(&[
        "theme",
        "set",
        "CyberPunkRED",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert_eq!(
        std::fs::read_to_string(sb.state().join("current/theme.name"))
            .expect("theme.name")
            .trim(),
        "cyber-punk-red"
    );
}

#[test]
fn set_renders_the_builtin_templates_from_the_palette() {
    let sb = Sandbox::new();
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

    let theme = sb.state().join("current/theme");
    let kitty = std::fs::read_to_string(theme.join("kitty.conf")).expect("kitty.conf");
    assert!(kitty.contains("background       #1a1b26"));

    let hyprland = std::fs::read_to_string(theme.join("hyprland.lua")).expect("hyprland.lua");
    assert!(hyprland.contains("local active = \"#7aa2f7\""));
    assert!(hyprland.contains("hl.config"));
}

#[test]
fn get_names_the_applied_theme() {
    let sb = Sandbox::new();
    sb.theme("tokyo-night");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    let run = sb.riso(&["theme", "get"]);
    assert!(run.status.success());
    assert!(out(&run).contains("tokyo-night"));
}

#[test]
fn an_exactly_named_theme_beats_loose_lookalikes() {
    let sb = Sandbox::new();
    sb.theme("rose-pine");
    sb.theme("rosepine");
    let run = sb.riso(&[
        "theme",
        "set",
        "RosePine",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    assert!(run.status.success());
    assert_eq!(
        std::fs::read_to_string(sb.state().join("current/theme.name"))
            .expect("theme.name")
            .trim(),
        "rosepine"
    );
}

#[test]
fn an_ambiguous_loose_name_is_refused() {
    let sb = Sandbox::new();
    sb.theme("rose-pine");
    sb.theme("rose_pine");
    let run = sb.riso(&[
        "theme",
        "set",
        "RosePine",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    assert!(!run.status.success());
}
