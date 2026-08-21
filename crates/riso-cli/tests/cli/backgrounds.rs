//! Wallpapers through the CLI: set, get, mode, and the rotation link.

use crate::common::{err, out, Sandbox};

fn theme_with_backgrounds(sb: &Sandbox) {
    let dir = sb.theme("tokyo-night").join("backgrounds");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("1-first.png"), b"png").expect("write");
    std::fs::write(dir.join("2-second.png"), b"png").expect("write");
    let run = sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    assert!(run.status.success(), "stderr: {}", err(&run));
}

#[test]
fn set_points_the_current_link_at_the_image() {
    let sb = Sandbox::new();
    theme_with_backgrounds(&sb);
    let image = sb.write("wall.png", "png");
    let run = sb.riso(&[
        "backgrounds",
        "set",
        image.to_str().expect("utf8"),
        "--no-reload",
    ]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let target = std::fs::read_link(sb.state().join("current/background")).expect("link");
    assert_eq!(target, image);
}

#[test]
fn next_advances_through_the_theme_backgrounds() {
    let sb = Sandbox::new();
    theme_with_backgrounds(&sb);
    let first = std::fs::read_link(sb.state().join("current/background")).expect("link");
    let run = sb.riso(&["backgrounds", "next", "--no-reload"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let second = std::fs::read_link(sb.state().join("current/background")).expect("link");
    assert_ne!(first, second, "next should move the link");
}

#[test]
fn mode_roundtrips_and_get_reports_both() {
    let sb = Sandbox::new();
    theme_with_backgrounds(&sb);
    let run = sb.riso(&["backgrounds", "mode", "fit"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let run = sb.riso(&["backgrounds", "get"]);
    assert!(run.status.success());
    assert!(out(&run).contains("fit"), "stdout: {}", out(&run));
}
