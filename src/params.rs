use anyhow::{anyhow, Result};
use serde_json::{Map, Value};

/// How bare `key=value` values are interpreted.
#[derive(Debug, Clone, Copy)]
pub enum ParamMode {
    /// Parse as JSON if valid, otherwise treat as a string.
    Auto,
    /// Always treat the value as a plain string.
    Text,
    /// Require the value to be valid JSON.
    StrictJson,
}

fn parse_value(s: &str, mode: ParamMode) -> Result<Value> {
    match mode {
        ParamMode::Text => Ok(Value::String(s.to_owned())),
        ParamMode::Auto => {
            Ok(serde_json::from_str::<Value>(s).unwrap_or_else(|_| Value::String(s.to_owned())))
        }
        ParamMode::StrictJson => serde_json::from_str::<Value>(s)
            .map_err(|e| anyhow!("invalid JSON value `{s}`: {e}")),
    }
}

/// Build the request params object.
///
/// `--params-json` (when `Some`) wins and is parsed as-is (must be a JSON
/// object). Otherwise each `key=value` pair is inserted into an object, with
/// the value interpreted per `mode`. An empty pair list yields `{}`.
pub fn parse_params(
    params_json: Option<&str>,
    pairs: &[String],
    mode: ParamMode,
) -> Result<Value> {
    if let Some(json) = params_json {
        let v: Value = serde_json::from_str(json)
            .map_err(|e| anyhow!("invalid JSON for --params-json: {e}"))?;
        if !v.is_object() {
            return Err(anyhow!("--params-json must be a JSON object"));
        }
        return Ok(v);
    }

    let mut obj = Map::new();
    for item in pairs {
        let (k, val) = item
            .split_once('=')
            .ok_or_else(|| anyhow!("expected key=value, got `{item}`"))?;
        if k.is_empty() {
            return Err(anyhow!("empty key in `{item}`"));
        }
        obj.insert(k.to_owned(), parse_value(val, mode)?);
    }
    Ok(Value::Object(obj))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_pairs_yield_empty_object() {
        let v = parse_params(None, &[], ParamMode::Auto).unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn auto_parses_numbers_bools_and_strings() {
        let pairs = vec!["amount_msat=100000".into(), "deschashonly=true".into(), "label=hello".into()];
        let v = parse_params(None, &pairs, ParamMode::Auto).unwrap();
        assert_eq!(v, json!({"amount_msat": 100000, "deschashonly": true, "label": "hello"}));
    }

    #[test]
    fn text_mode_keeps_strings() {
        let pairs = vec!["count=3".into(), "active=true".into()];
        let v = parse_params(None, &pairs, ParamMode::Text).unwrap();
        assert_eq!(v, json!({"count": "3", "active": "true"}));
    }

    #[test]
    fn strict_json_rejects_bare_words() {
        let pairs = vec!["label=hello".into()];
        assert!(parse_params(None, &pairs, ParamMode::StrictJson).is_err());
    }

    #[test]
    fn params_json_passthrough() {
        let v = parse_params(Some(r#"{"bolt11":"lnbc1"}"#), &[], ParamMode::Auto).unwrap();
        assert_eq!(v, json!({"bolt11": "lnbc1"}));
    }

    #[test]
    fn params_json_must_be_object() {
        assert!(parse_params(Some("[1,2,3]"), &[], ParamMode::Auto).is_err());
    }

    #[test]
    fn missing_equals_errors() {
        assert!(parse_params(None, &["nokey".into()], ParamMode::Auto).is_err());
    }
}
