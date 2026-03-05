use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("overview"))
        .stdout(predicate::str::contains("trace"));
}

#[test]
fn test_version_output() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_overview_requires_tokens() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tokens"));
}

#[test]
fn test_trace_requires_tokens_and_target() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["trace"])
        .assert()
        .failure();
}

#[test]
fn test_tokens_parses_k_suffix() {
    Command::cargo_bin("cxpak")
        .unwrap()
        .args(["overview", "--tokens", "50k"])
        .assert();
}
