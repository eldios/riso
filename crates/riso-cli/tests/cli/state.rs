//! The ownership store through the CLI: partial restore and the full
//! uninstall.

use crate::common::{err, out, Sandbox};

#[test]
fn restore_path_puts_back_only_the_named_file() {
    let sb = Sandbox::new();
    sb.fake_bin("kitty");
    sb.fake_bin("ghostty");
    sb.write(".config/kitty/kitty.conf", "font_size 12\n");
    sb.write(".config/ghostty/config", "font-size = 12\n");
    let run = sb.riso(&["config", "wire", "kitty", "ghostty", "--yes"]);
    assert!(run.status.success(), "stderr: {}", err(&run));

    let kitty = sb.home().join(".config/kitty/kitty.conf");
    let run = sb.riso(&["restore", "--path", kitty.to_str().expect("utf8")]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert_eq!(
        std::fs::read_to_string(&kitty).expect("read"),
        "font_size 12\n"
    );
    assert!(
        std::fs::read_to_string(sb.home().join(".config/ghostty/config"))
            .expect("read")
            .contains("added by riso"),
        "the other file should keep its wiring"
    );
}

#[test]
fn uninstall_restores_configs_and_forgets_the_state() {
    let sb = Sandbox::new();
    sb.fake_bin("kitty");
    sb.write(".config/kitty/kitty.conf", "font_size 12\n");
    sb.theme("tokyo-night");
    sb.riso(&[
        "theme",
        "set",
        "tokyo-night",
        "--desktop",
        "none",
        "--no-reload",
    ]);
    sb.riso(&["config", "wire", "kitty", "--yes"]);

    let run = sb.riso(&["uninstall", "--yes"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert_eq!(
        std::fs::read_to_string(sb.home().join(".config/kitty/kitty.conf")).expect("read"),
        "font_size 12\n"
    );
    assert!(!sb.state().join("current").exists(), "state should be gone");
    let _ = out(&run);
}
