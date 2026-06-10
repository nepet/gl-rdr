use assert_cmd::Command;
use predicates::prelude::*;

fn glrdr() -> Command {
    Command::cargo_bin("glrdr").unwrap()
}

#[test]
fn help_lists_methods() {
    glrdr()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("cln.Node:"))
        .stdout(predicate::str::contains("getinfo"));
}

#[test]
fn help_for_method_shows_path() {
    glrdr()
        .args(["help", "pay"])
        .assert()
        .success()
        .stdout(predicate::str::contains("/cln.Node/Pay"))
        .stdout(predicate::str::contains("bolt11"));
}

#[test]
fn dash_h_shows_usage() {
    glrdr()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"))
        .stdout(predicate::str::contains("--grpc-uri"));
}

#[test]
fn unknown_method_errors_before_network() {
    glrdr()
        .arg("definitely_not_a_method")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown method"));
}

#[test]
fn missing_credentials_errors_clearly() {
    glrdr()
        .arg("getinfo")
        .env_remove("GL_CREDS")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no credentials"));
}

#[test]
fn streaming_method_is_rejected() {
    glrdr()
        .args(["--service", "greenlight.Node", "StreamLog"])
        .env_remove("GL_CREDS")
        .assert()
        .failure()
        .stderr(predicate::str::contains("streaming"));
}
