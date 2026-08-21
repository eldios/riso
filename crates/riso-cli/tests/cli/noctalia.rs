//! Noctalia as the running shell: the palette link wiring and the IPC
//! pokes every apply sends.

use crate::common::{err, out, Sandbox};

#[test]
fn theme_set_tells_noctalia_to_re_read_the_palette() {
    let sb = Sandbox::new();
    sb.logging_bin("noctalia");
    sb.fake_bin("hyprctl");
    sb.theme("tokyo-night");
    let run = sb.riso(&["theme", "set", "tokyo-night", "--desktop", "hyprland"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(
        sb.calls("noctalia")
            .contains("msg color-scheme-set custom riso"),
        "calls: {}",
        sb.calls("noctalia")
    );
}

#[test]
fn backgrounds_set_hands_noctalia_the_wallpaper() {
    let sb = Sandbox::new();
    sb.logging_bin("noctalia");
    sb.fake_bin("hyprctl");
    sb.theme("tokyo-night");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    let image = sb.write("wall.png", "png");
    let run = sb.riso(&["backgrounds", "set", image.to_str().expect("utf8")]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let calls = sb.calls("noctalia");
    assert!(calls.contains("msg wallpaper-set"), "calls: {calls}");
    assert!(calls.contains("wall.png"), "calls: {calls}");
}

#[test]
fn wire_links_the_palette_and_restore_unlinks_it() {
    let sb = Sandbox::new();
    sb.fake_bin("noctalia");
    let run = sb.riso(&["config", "wire", "noctalia", "--yes"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("1 wired"), "stdout: {}", out(&run));

    let link = sb.home().join(".config/noctalia/palettes/riso.json");
    let target = std::fs::read_link(&link).expect("symlink");
    assert!(
        target.ends_with(".local/state/riso/current/theme/noctalia.json"),
        "target: {}",
        target.display()
    );

    let run = sb.riso(&["config", "check", "noctalia"]);
    assert!(run.status.success(), "stdout: {}", out(&run));

    let run = sb.riso(&["restore"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(
        std::fs::symlink_metadata(&link).is_err(),
        "link survived restore"
    );
}

#[test]
fn an_unwired_palette_check_hands_out_the_ln_line() {
    let sb = Sandbox::new();
    sb.fake_bin("noctalia");
    let run = sb.riso(&["config", "check", "noctalia"]);
    assert!(!run.status.success());
    let text = out(&run);
    assert!(text.contains("ln -sfn"), "stdout: {text}");
    assert!(text.contains("noctalia.json"), "stdout: {text}");
}

#[test]
fn a_declarative_system_shows_the_link_without_creating_it() {
    let sb = Sandbox::new();
    sb.fake_bin("noctalia");
    let run = sb.riso_env(
        &["config", "wire", "noctalia", "--yes"],
        "RISO_DECLARATIVE",
        "1",
    );
    assert!(run.status.success());
    assert!(out(&run).contains("ln -sfn"), "stdout: {}", out(&run));
    assert!(
        std::fs::symlink_metadata(sb.home().join(".config/noctalia/palettes/riso.json")).is_err()
    );
}
