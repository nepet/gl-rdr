use prost::Message;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=proto");

    // protox is a pure-Rust protobuf compiler: no `protoc` needed for our
    // descriptor. `node.proto` imports `primitives.proto`; the "proto"
    // include dir resolves that bare import.
    let fds = protox::compile(
        ["proto/node.proto", "proto/greenlight.proto"],
        ["proto"],
    )
    .expect("failed to compile vendored protos with protox");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("descriptor.bin"), fds.encode_to_vec())
        .expect("failed to write descriptor.bin");
}
