use anyhow::{anyhow, bail, Result};
use prost::Message as _;
use prost_reflect::{DynamicMessage, FieldDescriptor, Kind, MapKey, MessageDescriptor, Value};
use serde_json::{Map, Value as Json};

// ---- JSON -> protobuf -------------------------------------------------------

/// Build and encode a request message from a JSON object.
pub fn json_to_bytes(desc: &MessageDescriptor, json: &Json) -> Result<Vec<u8>> {
    Ok(json_to_message(desc, json)?.encode_to_vec())
}

fn json_to_message(desc: &MessageDescriptor, json: &Json) -> Result<DynamicMessage> {
    let obj = json
        .as_object()
        .ok_or_else(|| anyhow!("params for `{}` must be a JSON object", desc.full_name()))?;
    let mut msg = DynamicMessage::new(desc.clone());
    for (key, val) in obj {
        let field = desc
            .get_field_by_name(key)
            .ok_or_else(|| anyhow!("unknown field `{key}` on `{}`", desc.full_name()))?;
        msg.set_field(&field, json_to_field(&field, val)?);
    }
    Ok(msg)
}

fn json_to_field(field: &FieldDescriptor, v: &Json) -> Result<Value> {
    if field.is_list() {
        let arr = v
            .as_array()
            .ok_or_else(|| anyhow!("field `{}` must be an array", field.name()))?;
        let items = arr
            .iter()
            .map(|e| json_to_scalar(field, &field.kind(), e))
            .collect::<Result<Vec<_>>>()?;
        return Ok(Value::List(items));
    }
    if field.is_map() {
        let entry = field.kind();
        let entry = entry
            .as_message()
            .ok_or_else(|| anyhow!("map field `{}` has no entry type", field.name()))?;
        let value_field = entry.map_entry_value_field();
        let obj = v
            .as_object()
            .ok_or_else(|| anyhow!("map field `{}` must be an object", field.name()))?;
        let mut map = std::collections::HashMap::new();
        for (k, val) in obj {
            map.insert(
                MapKey::String(k.clone()),
                json_to_scalar(&value_field, &value_field.kind(), val)?,
            );
        }
        return Ok(Value::Map(map));
    }
    json_to_scalar(field, &field.kind(), v)
}

fn json_to_scalar(field: &FieldDescriptor, kind: &Kind, v: &Json) -> Result<Value> {
    Ok(match kind {
        Kind::Double => Value::F64(as_f64(field, v)?),
        Kind::Float => Value::F32(as_f64(field, v)? as f32),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => Value::I32(as_i32(field, v)?),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => Value::I64(as_i64(field, v)?),
        Kind::Uint32 | Kind::Fixed32 => Value::U32(as_u32(field, v)?),
        Kind::Uint64 | Kind::Fixed64 => Value::U64(as_u64(field, v)?),
        Kind::Bool => Value::Bool(
            v.as_bool()
                .ok_or_else(|| anyhow!("field `{}` must be a boolean", field.name()))?,
        ),
        Kind::String => Value::String(
            v.as_str()
                .ok_or_else(|| anyhow!("field `{}` must be a string", field.name()))?
                .to_owned(),
        ),
        Kind::Bytes => {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow!("field `{}` must be a hex string", field.name()))?;
            Value::Bytes(hex::decode(s).map_err(|e| anyhow!("field `{}`: {e}", field.name()))?.into())
        }
        Kind::Enum(en) => match v {
            Json::String(name) => {
                let ev = en
                    .values()
                    .find(|val| val.name().eq_ignore_ascii_case(name))
                    .ok_or_else(|| anyhow!("field `{}`: unknown enum value `{name}`", field.name()))?;
                Value::EnumNumber(ev.number())
            }
            Json::Number(_) => Value::EnumNumber(as_i64(field, v)? as i32),
            _ => bail!("field `{}` must be an enum name or number", field.name()),
        },
        Kind::Message(m) => Value::Message(json_to_message(m, v)?),
    })
}

fn as_i32(field: &FieldDescriptor, v: &Json) -> Result<i32> {
    let n = as_i64(field, v)?;
    i32::try_from(n)
        .map_err(|_| anyhow!("field `{}`: value {n} out of range for int32", field.name()))
}

fn as_u32(field: &FieldDescriptor, v: &Json) -> Result<u32> {
    let n = as_u64(field, v)?;
    u32::try_from(n)
        .map_err(|_| anyhow!("field `{}`: value {n} out of range for uint32", field.name()))
}

fn as_i64(field: &FieldDescriptor, v: &Json) -> Result<i64> {
    match v {
        Json::Number(n) => n
            .as_i64()
            .ok_or_else(|| anyhow!("field `{}` is not a valid integer", field.name())),
        Json::String(s) => s
            .parse::<i64>()
            .map_err(|e| anyhow!("field `{}`: {e}", field.name())),
        _ => Err(anyhow!("field `{}` must be an integer", field.name())),
    }
}

fn as_u64(field: &FieldDescriptor, v: &Json) -> Result<u64> {
    match v {
        Json::Number(n) => n
            .as_u64()
            .ok_or_else(|| anyhow!("field `{}` is not a valid unsigned integer", field.name())),
        Json::String(s) => s
            .parse::<u64>()
            .map_err(|e| anyhow!("field `{}`: {e}", field.name())),
        _ => Err(anyhow!("field `{}` must be an unsigned integer", field.name())),
    }
}

fn as_f64(field: &FieldDescriptor, v: &Json) -> Result<f64> {
    v.as_f64()
        .ok_or_else(|| anyhow!("field `{}` must be a number", field.name()))
}

// ---- protobuf -> CLN-flavored JSON -----------------------------------------

/// Decode response bytes into CLN-flavored JSON: 64-bit ints as numbers,
/// bytes as hex, enums as names. Only populated fields are emitted.
pub fn bytes_to_cln_json(desc: &MessageDescriptor, bytes: &[u8]) -> Result<Json> {
    let msg = DynamicMessage::decode(desc.clone(), bytes)
        .map_err(|e| anyhow!("failed to decode `{}` response: {e}", desc.full_name()))?;
    Ok(message_to_cln_json(&msg))
}

fn message_to_cln_json(msg: &DynamicMessage) -> Json {
    let mut out = Map::new();
    for (field, value) in msg.fields() {
        out.insert(field.name().to_owned(), value_to_cln_json(&field.kind(), value));
    }
    Json::Object(out)
}

fn value_to_cln_json(kind: &Kind, value: &Value) -> Json {
    match value {
        Value::Bool(b) => Json::Bool(*b),
        Value::I32(n) => Json::from(*n),
        Value::I64(n) => Json::from(*n),
        Value::U32(n) => Json::from(*n),
        Value::U64(n) => Json::from(*n),
        Value::F32(n) => serde_json::Number::from_f64(*n as f64).map(Json::Number).unwrap_or(Json::Null),
        Value::F64(n) => serde_json::Number::from_f64(*n).map(Json::Number).unwrap_or(Json::Null),
        Value::String(s) => Json::String(s.clone()),
        Value::Bytes(b) => Json::String(hex::encode(b)),
        Value::EnumNumber(n) => match kind.as_enum().and_then(|e| e.get_value(*n)) {
            Some(ev) => Json::String(ev.name().to_owned()),
            None => Json::from(*n),
        },
        Value::Message(m) => message_to_cln_json(m),
        Value::List(items) => Json::Array(items.iter().map(|i| value_to_cln_json(kind, i)).collect()),
        Value::Map(map) => {
            let value_kind = kind
                .as_message()
                .map(|m| m.map_entry_value_field().kind());
            let mut obj = Map::new();
            for (k, v) in map {
                let key = match k {
                    MapKey::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                let vk = value_kind.clone().unwrap_or_else(|| kind.clone());
                obj.insert(key, value_to_cln_json(&vk, v));
            }
            Json::Object(obj)
        }
    }
}
