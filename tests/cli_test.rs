use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_output() {
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("overview"))
        .stdout(predicate::str::contains("trace"));
}

#[test]
fn test_version_output() {
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_overview_requires_tokens() {
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tokens"));
}

#[test]
fn test_trace_requires_tokens_and_target() {
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["trace"])
        .assert()
        .failure();
}

#[test]
fn test_tokens_parses_k_suffix() {
    Command::new(assert_cmd::cargo_bin!("cxpak"))
        .args(["overview", "--tokens", "50k"])
        .assert()
        .success();
}
