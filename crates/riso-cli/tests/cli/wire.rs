//! `riso config wire` end to end: writing the include, counting what was
//! left alone, refusing edits on declarative systems, and the restore
//! that puts every byte back.

use crate::common::{err, out, Sandbox};

const MARKER: &str = "added by riso config wire";

#[test]
fn wire_creates_a_missing_config_and_restore_puts_it_back() {
    let sb = Sandbox::new();
    sb.fake_bin("kitty");

    let run = sb.riso(&["config", "wire", "kitty", "--yes"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("1 wired"), "stdout: {}", out(&run));

    let config = sb.home().join(".config/kitty/kitty.conf");
    let written = std::fs::read_to_string(&config).expect("written");
    assert!(written.contains(MARKER));
    assert!(written.contains("include "));
    assert!(written.contains("/riso/current/theme/kitty.conf"));

    let run = sb.riso(&["restore"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(
        !config.exists() || !std::fs::read_to_string(&config).unwrap().contains(MARKER),
        "restore left riso's line in place"
    );
}

#[test]
fn a_second_run_counts_the_wired_file_as_left_alone() {
    let sb = Sandbox::new();
    sb.fake_bin("kitty");
    sb.riso(&["config", "wire", "kitty", "--yes"]);
    let run = sb.riso(&["config", "wire", "kitty", "--yes"]);
    assert!(run.status.success());
    let text = out(&run);
    assert!(
        text.contains("0 wired, 1 left as they were"),
        "stdout: {text}"
    );
    assert!(text.contains("already wired"));
}

#[test]
fn an_existing_config_is_appended_not_replaced() {
    let sb = Sandbox::new();
    sb.fake_bin("kitty");
    sb.write(".config/kitty/kitty.conf", "font_size 12\n");
    let run = sb.riso(&["config", "wire", "kitty", "--yes"]);
    assert!(run.status.success());
    let written =
        std::fs::read_to_string(sb.home().join(".config/kitty/kitty.conf")).expect("read");
    assert!(written.starts_with("font_size 12\n"));
    assert!(written.contains(MARKER));
}

#[test]
fn a_declarative_system_only_shows_the_lines() {
    let sb = Sandbox::new();
    sb.fake_bin("kitty");
    sb.write(".config/kitty/kitty.conf", "font_size 12\n");
    let run = sb.riso_env(
        &["config", "wire", "kitty", "--yes"],
        "RISO_DECLARATIVE",
        "1",
    );
    assert!(run.status.success());
    assert!(out(&run).contains("declarative system"));
    assert_eq!(
        std::fs::read_to_string(sb.home().join(".config/kitty/kitty.conf")).expect("read"),
        "font_size 12\n"
    );
}

#[test]
fn a_conflicting_config_is_shown_not_edited() {
    let sb = Sandbox::new();
    sb.fake_bin("btop");
    sb.write(".config/btop/btop.conf", "color_theme = \"Default\"\n");
    let run = sb.riso(&["config", "wire", "btop", "--yes"]);
    assert!(run.status.success());
    assert!(out(&run).contains("hand-placed"), "stdout: {}", out(&run));
    assert_eq!(
        std::fs::read_to_string(sb.home().join(".config/btop/btop.conf")).expect("read"),
        "color_theme = \"Default\"\n"
    );
}

#[test]
fn naming_an_unknown_app_lists_the_wirable_ones() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "wire", "nope", "--yes"]);
    assert!(!run.status.success());
    assert!(err(&run).contains("kitty"), "stderr: {}", err(&run));
}
