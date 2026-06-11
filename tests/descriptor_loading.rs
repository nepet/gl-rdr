use prost::Message as _;
use std::path::{Path, PathBuf};

/// Compile the addon fixture to a serialized FileDescriptorSet.
fn addon_bytes() -> Vec<u8> {
    protox::compile(["tests/fixtures/addon.proto"], ["tests/fixtures"])
        .unwrap()
        .encode_to_vec()
}

#[test]
fn pool_with_set_resolves_addon_and_bundled_methods() {
    let pool = gl_rdr::descriptor::pool_with_set(&addon_bytes()).unwrap();

    // Addon method resolves via explicit path.
    let addon = gl_rdr::descriptor::resolve(&pool, "addon.Addon/Ping", None).unwrap();
    assert_eq!(gl_rdr::descriptor::grpc_path(&addon), "/addon.Addon/Ping");

    // Bundled methods still resolve from the merged pool.
    let bundled = gl_rdr::descriptor::resolve(&pool, "getinfo", None).unwrap();
    assert_eq!(gl_rdr::descriptor::grpc_path(&bundled), "/cln.Node/Getinfo");
}

#[test]
fn pool_with_set_rejects_garbage_bytes() {
    let err = gl_rdr::descriptor::pool_with_set(&[0xff, 0xff, 0xff, 0xff]).unwrap_err();
    assert!(
        err.to_string().contains("descriptor set"),
        "error was: {err}"
    );
}

/// The vendored `node.proto` with one extra rpc grafted onto `cln.Node`,
/// compiled to a descriptor set. Loading it should OVERRIDE the bundled
/// `node.proto`, so the grafted method becomes reachable.
fn extended_node_bytes() -> Vec<u8> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("override_proto");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::fs::read_to_string("proto/node.proto").unwrap();
    let extended = src.replacen(
        "service Node {",
        "service Node {\n  rpc OverrideProbe (GetinfoRequest) returns (GetinfoResponse);",
        1,
    );
    assert!(extended.contains("OverrideProbe"), "graft failed");
    std::fs::write(dir.join("node.proto"), &extended).unwrap();
    std::fs::copy("proto/primitives.proto", dir.join("primitives.proto")).unwrap();
    protox::compile(["node.proto"], [dir.to_str().unwrap()])
        .unwrap()
        .encode_to_vec()
}

fn write_tmp(name: &str, bytes: &[u8]) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn effective_pool_loads_from_file() {
    let path = write_tmp("addon.bin", &addon_bytes());
    let pool = gl_rdr::descriptor::effective_pool(Some(&path)).unwrap();
    assert!(gl_rdr::descriptor::resolve(&pool, "addon.Addon/Ping", None).is_ok());
}

#[test]
fn effective_pool_none_is_bundled_only() {
    let pool = gl_rdr::descriptor::effective_pool(None).unwrap();
    assert!(gl_rdr::descriptor::resolve(&pool, "getinfo", None).is_ok());
    assert!(gl_rdr::descriptor::resolve(&pool, "addon.Addon/Ping", None).is_err());
}

#[test]
fn effective_pool_missing_file_errors() {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("effective_pool_missing_file_errors__does-not-exist.bin");
    let err = gl_rdr::descriptor::effective_pool(Some(&path)).unwrap_err();
    assert!(
        err.to_string().contains("failed to read descriptor set from"),
        "error was: {err}"
    );
}

#[test]
fn effective_pool_garbage_file_errors() {
    let path = write_tmp("garbage.bin", &[0xff, 0xff, 0xff, 0xff]);
    let err = gl_rdr::descriptor::effective_pool(Some(&path)).unwrap_err();
    assert!(
        err.to_string().contains("failed to load descriptor set from"),
        "error was: {err}"
    );
}

#[test]
fn external_node_proto_overrides_bundled() {
    let pool = gl_rdr::descriptor::pool_with_set(&extended_node_bytes()).unwrap();
    // The grafted method is reachable — the external node.proto replaced the
    // bundled one on the name clash.
    let probe = gl_rdr::descriptor::resolve(&pool, "cln.Node/OverrideProbe", None).unwrap();
    assert_eq!(gl_rdr::descriptor::grpc_path(&probe), "/cln.Node/OverrideProbe");
    // Existing cln.Node methods survive (the external file is a superset).
    assert!(gl_rdr::descriptor::resolve(&pool, "getinfo", None).is_ok());
    // Bundled-only files (greenlight.proto) are preserved, not dropped.
    assert!(gl_rdr::descriptor::resolve(&pool, "Configure", Some("greenlight.Node")).is_ok());
}
