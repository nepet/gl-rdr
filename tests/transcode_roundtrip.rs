use prost::Message;
use prost_reflect::DescriptorPool;
use serde_json::json;

fn fixture_pool() -> DescriptorPool {
    let fds = protox::compile(["tests/fixtures/transcode.proto"], ["tests/fixtures"]).unwrap();
    DescriptorPool::decode(fds.encode_to_vec().as_slice()).unwrap()
}

#[test]
fn json_to_bytes_then_back_is_cln_flavored() {
    let pool = fixture_pool();
    let sample = pool.get_message_by_name("fixture.Sample").unwrap();

    let input = json!({
        "name": "alice",
        "big": 18446744073709551615u64,   // u64::MAX, must stay a number
        "small": -7,
        "flag": true,
        "blob": "deadbeef",                // hex in
        "color": "BLUE",
        "inner": {"note": "hi"},
        "tags": ["a", "b"],
        "blobs": ["00ff", "01"]
    });

    let bytes = gl_rdr::transcode::json_to_bytes(&sample, &input).unwrap();
    let back = gl_rdr::transcode::bytes_to_cln_json(&sample, &bytes).unwrap();

    assert_eq!(back["name"], json!("alice"));
    assert_eq!(back["big"], json!(18446744073709551615u64)); // number, not string
    assert_eq!(back["small"], json!(-7));
    assert_eq!(back["flag"], json!(true));
    assert_eq!(back["blob"], json!("deadbeef"));            // hex out
    assert_eq!(back["color"], json!("BLUE"));               // enum name
    assert_eq!(back["inner"], json!({"note": "hi"}));
    assert_eq!(back["tags"], json!(["a", "b"]));
    assert_eq!(back["blobs"], json!(["00ff", "01"]));
}

#[test]
fn unknown_field_is_rejected() {
    let pool = fixture_pool();
    let sample = pool.get_message_by_name("fixture.Sample").unwrap();
    let err = gl_rdr::transcode::json_to_bytes(&sample, &json!({"nope": 1})).unwrap_err();
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn out_of_range_int32_is_rejected() {
    let pool = fixture_pool();
    let sample = pool.get_message_by_name("fixture.Sample").unwrap();
    // i32::MAX + 1 must not silently wrap to a negative number.
    let err = gl_rdr::transcode::json_to_bytes(&sample, &serde_json::json!({"small": 2147483648u64}))
        .unwrap_err();
    assert!(err.to_string().contains("out of range"));
}
