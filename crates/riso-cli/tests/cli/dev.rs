//! The theme author's tools: palette resolution and the standalone
//! render.

use crate::common::{err, out, Sandbox};

#[test]
fn palette_prints_the_resolved_pairs() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night");
    let run = sb.riso(&["dev", "palette", "--theme", dir.to_str().expect("utf8")]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let text = out(&run);
    assert!(text.contains("accent"));
    assert!(text.contains("#7aa2f7"));
}

#[test]
fn render_writes_the_configs_into_the_named_directory() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night");
    let dest = sb.home().join("rendered");
    let run = sb.riso(&[
        "dev",
        "render",
        "--theme",
        dir.to_str().expect("utf8"),
        "--out",
        dest.to_str().expect("utf8"),
    ]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(dest.join("kitty.conf").is_file());
    assert!(dest.join("hyprland.lua").is_file());
}

#[test]
fn a_missing_theme_directory_is_an_error() {
    let sb = Sandbox::new();
    let run = sb.riso(&["dev", "palette", "--theme", "/nope"]);
    assert!(!run.status.success());
}
