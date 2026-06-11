use assert_cmd::Command;
use predicates::prelude::*;
use prost::Message as _;
use std::path::{Path, PathBuf};

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

fn write_addon_bin(name: &str) -> PathBuf {
    let fds = protox::compile(["tests/fixtures/addon.proto"], ["tests/fixtures"]).unwrap();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, fds.encode_to_vec()).unwrap();
    path
}

#[test]
fn descriptor_flag_lists_addon_service_in_help() {
    let bin = write_addon_bin("cli_addon.bin");
    glrdr()
        .args(["--descriptor", bin.to_str().unwrap(), "help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("addon.Addon:"))
        .stdout(predicate::str::contains("ping"));
}

#[test]
fn descriptor_flag_describes_addon_method() {
    let bin = write_addon_bin("cli_addon_desc.bin");
    glrdr()
        .args(["--descriptor", bin.to_str().unwrap(), "help", "addon.Addon/Ping"])
        .assert()
        .success()
        .stdout(predicate::str::contains("/addon.Addon/Ping"))
        .stdout(predicate::str::contains("nonce"));
}

#[test]
fn descriptor_env_var_is_honored() {
    let bin = write_addon_bin("cli_addon_env.bin");
    glrdr()
        .arg("help")
        .env("GL_DESCRIPTOR", bin.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("addon.Addon:"));
}

/// Vendored node.proto with an extra rpc grafted on, compiled to a .bin that
/// overrides the bundled node.proto.
fn write_override_node_bin(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.d"));
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::fs::read_to_string("proto/node.proto").unwrap();
    let extended = src.replacen(
        "service Node {",
        "service Node {\n  rpc OverrideProbe (GetinfoRequest) returns (GetinfoResponse);",
        1,
    );
    std::fs::write(dir.join("node.proto"), &extended).unwrap();
    std::fs::copy("proto/primitives.proto", dir.join("primitives.proto")).unwrap();
    let fds = protox::compile(["node.proto"], [dir.to_str().unwrap()]).unwrap();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, fds.encode_to_vec()).unwrap();
    path
}

#[test]
fn descriptor_overrides_bundled_node_proto() {
    let bin = write_override_node_bin("cli_override.bin");
    glrdr()
        .args(["--descriptor", bin.to_str().unwrap(), "help", "cln.Node/OverrideProbe"])
        .assert()
        .success()
        .stdout(predicate::str::contains("/cln.Node/OverrideProbe"))
        .stderr(predicate::str::contains("overrides bundled `node.proto`"));
}

#[test]
fn bad_descriptor_path_errors_clearly() {
    glrdr()
        .args(["--descriptor", "/no/such/file.bin", "help"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to read descriptor set from"));
}
