//! The hidden carousel-data feed the pickers read: rows and the
//! current markers.

use crate::common::{err, out, Sandbox};

#[test]
fn themes_rows_carry_every_installed_theme() {
    let sb = Sandbox::new();
    sb.theme("tokyo-night");
    sb.theme("rose-pine");
    let run = sb.riso(&["carousel-data", "themes"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let text = out(&run);
    assert!(text.contains("tokyo-night"));
    assert!(text.contains("rose-pine"));
}

#[test]
fn current_names_the_applied_theme() {
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
    let run = sb.riso(&["carousel-data", "themes", "--current"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("tokyo-night"));
}

#[test]
fn backgrounds_rows_carry_the_theme_wallpapers() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night").join("backgrounds");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("1-first.png"), b"png").expect("write");
    std::fs::write(dir.join("2-second.png"), b"png").expect("write");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    let run = sb.riso(&["carousel-data", "backgrounds"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let text = out(&run);
    assert!(text.contains("1-first"), "stdout: {text}");
    assert!(text.contains("2-second"));
}

#[test]
fn backgrounds_current_names_the_linked_wallpaper() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night").join("backgrounds");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("1-first.png"), b"png").expect("write");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    let run = sb.riso(&["carousel-data", "backgrounds", "--current"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("1-first"), "stdout: {}", out(&run));
}
