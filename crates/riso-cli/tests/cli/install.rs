//! Installing, updating and removing themes from a git source, all
//! against local repositories.

use crate::common::{err, out, Sandbox};

fn with_git(sb: &Sandbox) {
    sb.real_bin("git");
}

#[test]
fn install_clones_a_git_source_into_the_user_themes() {
    let sb = Sandbox::new();
    with_git(&sb);
    let repo = sb.git_theme_repo("tokyo-night");
    let run = sb.riso(&["theme", "install", &format!("file://{}", repo.display())]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    let run = sb.riso(&["theme", "list"]);
    assert!(out(&run).contains("tokyo-night"), "stdout: {}", out(&run));
}

#[test]
fn update_pulls_what_upstream_gained_and_quiet_says_nothing() {
    let sb = Sandbox::new();
    with_git(&sb);
    let repo = sb.git_theme_repo("tokyo-night");
    sb.riso(&["theme", "install", &format!("file://{}", repo.display())]);

    std::fs::write(repo.join("extra.css"), "body {}\n").expect("write");
    sb.git(&repo, &["add", "."]);
    sb.git(&repo, &["commit", "-q", "-m", "extra"]);

    let run = sb.riso(&["theme", "update", "-q"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).is_empty(), "quiet printed: {}", out(&run));
    assert!(sb
        .home()
        .join(".config/riso/themes/tokyo-night/extra.css")
        .is_file());
}

#[test]
fn update_without_quiet_reports_each_theme() {
    let sb = Sandbox::new();
    with_git(&sb);
    let repo = sb.git_theme_repo("tokyo-night");
    sb.riso(&["theme", "install", &format!("file://{}", repo.display())]);
    let run = sb.riso(&["theme", "update"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(out(&run).contains("tokyo-night"), "stdout: {}", out(&run));
}

#[test]
fn remove_deletes_the_installed_theme() {
    let sb = Sandbox::new();
    with_git(&sb);
    let repo = sb.git_theme_repo("tokyo-night");
    sb.riso(&["theme", "install", &format!("file://{}", repo.display())]);
    let run = sb.riso(&["theme", "remove", "tokyo-night"]);
    assert!(run.status.success(), "stderr: {}", err(&run));
    assert!(!sb.home().join(".config/riso/themes/tokyo-night").exists());
}

#[test]
fn validate_praises_a_sound_theme_and_flags_a_broken_one() {
    let sb = Sandbox::new();
    let dir = sb.theme("tokyo-night");
    let run = sb.riso(&["theme", "validate", dir.to_str().expect("utf8")]);
    assert!(run.status.success(), "stderr: {}", err(&run));

    let broken = sb.home().join("broken-theme");
    std::fs::create_dir_all(&broken).expect("mkdir");
    let run = sb.riso(&["theme", "validate", broken.to_str().expect("utf8")]);
    assert!(!run.status.success());
}
