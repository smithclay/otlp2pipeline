// src/convert.rs
//! Shared conversion utilities

use vrl::value::Value;

/// Convert VRL Value to serde_json::Value.
/// Returns None for values that cannot be represented in JSON (NaN, Infinity).
/// Null values in objects are omitted (not serialized).
pub fn vrl_value_to_json(v: &Value) -> Option<serde_json::Value> {
    match v {
        Value::Bytes(b) => Some(serde_json::Value::String(
            String::from_utf8_lossy(b).to_string(),
        )),
        Value::Integer(i) => Some(serde_json::Value::Number((*i).into())),
        Value::Float(f) => {
            let inner = f.into_inner();
            match serde_json::Number::from_f64(inner) {
                Some(n) => Some(serde_json::Value::Number(n)),
                None => {
                    tracing::debug!(
                        value = %inner,
                        "dropping non-finite float (NaN/Infinity) during JSON conversion"
                    );
                    None
                }
            }
        }
        Value::Boolean(b) => Some(serde_json::Value::Bool(*b)),
        Value::Null => Some(serde_json::Value::Null),
        Value::Array(arr) => {
            let items: Vec<_> = arr.iter().filter_map(vrl_value_to_json).collect();
            Some(serde_json::Value::Array(items))
        }
        Value::Object(map) => {
            // Skip null values in objects - they represent deleted/absent fields
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .filter(|(_, v)| !matches!(v, Value::Null))
                .filter_map(|(k, v)| vrl_value_to_json(v).map(|jv| (k.to_string(), jv)))
                .collect();
            Some(serde_json::Value::Object(obj))
        }
        _ => None,
    }
}

/// Convert VRL Value to serde_json::Value, using null for unconvertible values.
pub fn vrl_value_to_json_lossy(v: &Value) -> serde_json::Value {
    vrl_value_to_json(v).unwrap_or(serde_json::Value::Null)
}
