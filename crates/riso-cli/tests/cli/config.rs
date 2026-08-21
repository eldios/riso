//! The config file through the CLI: list, get, set, and the roundtrip
//! into config.toml.

use crate::common::{err, out, Sandbox};

#[test]
fn list_prints_every_option_with_defaults() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "list"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let text = out(&run);
    assert!(text.contains("output"));
    assert!(text.contains("omarchy-themes"));
}

#[test]
fn set_then_get_roundtrips_through_the_file() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "set", "output", "json"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let run = sb.riso(&["config", "get", "output"]);
    assert!(run.status.success());
    assert!(out(&run).contains("json"));
    let file =
        std::fs::read_to_string(sb.home().join(".config/riso/config.toml")).expect("config.toml");
    assert!(file.contains("json"));
}

#[test]
fn an_unknown_key_is_an_error() {
    let sb = Sandbox::new();
    let run = sb.riso(&["config", "get", "nope"]);
    assert!(!run.status.success());
}
