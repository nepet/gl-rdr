fn main() {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/descriptor.bin"));
    println!("descriptor bytes: {}", bytes.len());
}
