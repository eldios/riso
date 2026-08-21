//! `-o json` and `-o yaml` must stay machine-readable on every listing.

use crate::common::{err, out, Sandbox};

#[test]
fn theme_list_as_json_parses_and_names_the_themes() {
    let sb = Sandbox::new();
    sb.theme("tokyo-night");
    let run = sb.riso(&["-o", "json", "theme", "list"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let parsed: serde_json::Value = serde_json::from_str(&out(&run)).expect("json");
    assert!(parsed.to_string().contains("tokyo-night"));
}

#[test]
fn check_as_json_carries_the_verdict_fields() {
    let sb = Sandbox::new();
    sb.fake_bin("git");
    let run = sb.riso(&["-o", "json", "config", "check", "git"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let parsed: serde_json::Value = serde_json::from_str(&out(&run)).expect("json");
    let checks = parsed.as_array().expect("array");
    assert!(checks.iter().any(|c| c["name"] == "git" && c["ok"] == true));
}

#[test]
fn apps_as_yaml_parses_with_provenance() {
    let sb = Sandbox::new();
    let run = sb.riso(&["-o", "yaml", "config", "apps"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let parsed: serde_norway::Value = serde_norway::from_str(&out(&run)).expect("yaml");
    assert!(serde_norway::to_string(&parsed)
        .expect("yaml")
        .contains("riso built-in"));
}

#[test]
fn a_config_toml_output_default_is_honored() {
    let sb = Sandbox::new();
    sb.theme("tokyo-night");
    sb.riso(&["config", "set", "output", "json"]);
    let run = sb.riso(&["theme", "list"]);
    assert!(run.status.success());
    assert!(serde_json::from_str::<serde_json::Value>(&out(&run)).is_ok());
}

#[test]
fn theme_list_as_yaml_parses() {
    let sb = Sandbox::new();
    sb.theme("tokyo-night");
    let run = sb.riso(&["-o", "yaml", "theme", "list"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let parsed: serde_norway::Value = serde_norway::from_str(&out(&run)).expect("yaml");
    assert!(serde_norway::to_string(&parsed)
        .expect("yaml")
        .contains("tokyo-night"));
}
