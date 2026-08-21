//! `riso config apps`: provenance rows, including both Hyprland config
//! spellings when the theme carries the hyprlang one.

use crate::common::{err, out, Sandbox};

#[test]
fn without_a_theme_only_builtins_show() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "apps"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let text = out(&run);
    assert!(text.contains("riso built-in"));
    assert!(text.contains("kitty"));
}

#[test]
fn both_hyprland_spellings_show_and_neither_claims_an_override() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night");
    std::fs::write(dir.join("hyprland.conf"), "# theme borders\n").expect("write");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);

    let run = sb.riso(&["config", "apps"]);
    assert!(run.status.success());
    let text = out(&run);
    let hyprland: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("hyprland "))
        .collect();
    assert_eq!(hyprland.len(), 2, "stdout: {text}");
    assert!(text.contains("hyprland.conf"));
    assert!(text.contains("hyprland.lua"));
    for line in hyprland {
        assert!(!line.contains("overrides"), "line: {line}");
    }
}

#[test]
fn a_theme_file_over_a_template_is_labeled_an_override() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night");
    std::fs::write(dir.join("kitty.conf"), "# hand-tuned\n").expect("write");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);

    let run = sb.riso(&["config", "apps"]);
    let text = out(&run);
    let kitty: Vec<&str> = text.lines().filter(|l| l.starts_with("kitty")).collect();
    assert_eq!(kitty.len(), 1, "stdout: {text}");
    assert!(kitty[0].contains("overrides a template"));
}
