// src/decode/hec/logs.rs
use bytes::Bytes;
use serde::Deserialize;
use std::collections::HashMap;
use vrl::value::{KeyString, ObjectMap, Value};

use crate::decode::otlp::common::serde_json_to_vrl;

/// Maximum payload size (10MB, matches decompression limit)
const MAX_HEC_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Maximum events per request to prevent DoS
const MAX_HEC_EVENTS: usize = 10_000;

#[derive(Debug)]
pub enum HecDecodeError {
    Json(String),
    InvalidTimestamp(String),
    PayloadTooLarge(usize),
    TooManyEvents(usize),
}

impl std::fmt::Display for HecDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HecDecodeError::Json(e) => write!(f, "JSON parse error: {}", e),
            HecDecodeError::InvalidTimestamp(e) => write!(f, "invalid timestamp: {}", e),
            HecDecodeError::PayloadTooLarge(size) => {
                write!(
                    f,
                    "payload too large: {} bytes exceeds {} MB limit",
                    size,
                    MAX_HEC_BODY_SIZE / 1024 / 1024
                )
            }
            HecDecodeError::TooManyEvents(count) => {
                write!(
                    f,
                    "too many events: {} exceeds {} event limit",
                    count, MAX_HEC_EVENTS
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct HecEvent {
    #[serde(default)]
    pub time: Option<f64>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub sourcetype: Option<String>,
    #[serde(default)]
    pub event: serde_json::Value,
    #[serde(default)]
    pub fields: Option<HashMap<String, serde_json::Value>>,
}

/// Convert epoch seconds (f64) to integer milliseconds (i64) with overflow protection.
///
/// HEC timestamps are float epoch seconds. We convert to milliseconds
/// to match OTLP timestamp format for Cloudflare Pipelines compatibility.
pub fn safe_epoch_to_millis(time: f64) -> Result<i64, HecDecodeError> {
    // Reject non-finite values
    if !time.is_finite() {
        return Err(HecDecodeError::InvalidTimestamp("non-finite value".into()));
    }

    // Reject negative timestamps (pre-Unix epoch)
    if time < 0.0 {
        return Err(HecDecodeError::InvalidTimestamp(
            "negative timestamp".into(),
        ));
    }

    // Convert to milliseconds
    let millis = time * 1000.0;

    // Reject timestamps beyond i64 range
    if millis > i64::MAX as f64 {
        return Err(HecDecodeError::InvalidTimestamp("exceeds i64 max".into()));
    }

    Ok(millis.trunc() as i64)
}

/// Get current time in milliseconds (for missing timestamps)
#[cfg(not(target_arch = "wasm32"))]
fn current_time_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
fn current_time_millis() -> i64 {
    worker::Date::now().as_millis() as i64
}

/// Convert event body to VRL Value (string pass-through, other types JSON-encoded)
fn event_to_body(event: serde_json::Value) -> Value {
    match event {
        serde_json::Value::String(s) => Value::Bytes(Bytes::from(s)),
        serde_json::Value::Null => Value::Null,
        other => Value::Bytes(Bytes::from(other.to_string())),
    }
}

/// Convert HecEvent to VRL Value for transformation
fn hec_event_to_vrl(event: HecEvent) -> Result<Value, HecDecodeError> {
    let mut map = ObjectMap::new();

    // Timestamp (milliseconds to match OTLP format)
    let timestamp = match event.time {
        Some(t) => safe_epoch_to_millis(t)?,
        None => current_time_millis(),
    };
    map.insert("timestamp".into(), Value::Integer(timestamp));
    map.insert("observed_timestamp".into(), Value::Integer(timestamp));

    // Body
    map.insert("body".into(), event_to_body(event.event));

    // Metadata fields (VRL will build resource_attributes from these)
    if let Some(host) = event.host {
        map.insert("host".into(), Value::Bytes(Bytes::from(host)));
    }
    if let Some(source) = event.source {
        map.insert("source".into(), Value::Bytes(Bytes::from(source)));
    }
    if let Some(sourcetype) = event.sourcetype {
        map.insert("sourcetype".into(), Value::Bytes(Bytes::from(sourcetype)));
    }

    // Fields
    if let Some(fields) = event.fields {
        let fields_map: ObjectMap = fields
            .into_iter()
            .map(|(k, v)| (KeyString::from(k), serde_json_to_vrl(v)))
            .collect();
        map.insert("fields".into(), Value::Object(fields_map));
    }

    Ok(Value::Object(map))
}

/// Decode HEC NDJSON payload into VRL Values
pub fn decode_hec_logs(body: Bytes) -> Result<Vec<Value>, HecDecodeError> {
    // Check payload size limit
    if body.len() > MAX_HEC_BODY_SIZE {
        return Err(HecDecodeError::PayloadTooLarge(body.len()));
    }

    let text = std::str::from_utf8(&body)
        .map_err(|e| HecDecodeError::Json(format!("invalid UTF-8: {}", e)))?;

    let mut values = Vec::new();

    for line in text.split('\n') {
        // Trim \r for Windows line endings
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }

        // Check event count limit
        if values.len() >= MAX_HEC_EVENTS {
            return Err(HecDecodeError::TooManyEvents(values.len() + 1));
        }

        let event: HecEvent = serde_json::from_str(line)
            .map_err(|e| HecDecodeError::Json(format!("line parse error: {}", e)))?;

        values.push(hec_event_to_vrl(event)?);
    }

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_to_millis_basic() {
        assert_eq!(safe_epoch_to_millis(0.0).unwrap(), 0);
        assert_eq!(safe_epoch_to_millis(1.0).unwrap(), 1000);
        assert_eq!(safe_epoch_to_millis(1.5).unwrap(), 1500);
    }

    #[test]
    fn test_epoch_to_millis_preserves_subsecond() {
        // Sub-second precision is preserved as milliseconds
        assert_eq!(safe_epoch_to_millis(1703265600.123).unwrap(), 1703265600123);
        assert_eq!(safe_epoch_to_millis(1703265600.999).unwrap(), 1703265600999);
        assert_eq!(safe_epoch_to_millis(0.001).unwrap(), 1);
        assert_eq!(safe_epoch_to_millis(0.5).unwrap(), 500);
    }

    #[test]
    fn test_epoch_to_millis_rejects_negative() {
        assert!(safe_epoch_to_millis(-1.0).is_err());
    }

    #[test]
    fn test_epoch_to_millis_rejects_infinity() {
        assert!(safe_epoch_to_millis(f64::INFINITY).is_err());
        assert!(safe_epoch_to_millis(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn test_epoch_to_millis_rejects_nan() {
        assert!(safe_epoch_to_millis(f64::NAN).is_err());
    }

    #[test]
    fn test_decode_single_event() {
        let body = Bytes::from(r#"{"time": 1703265600.123, "event": "test log"}"#);
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 1);

        if let Value::Object(map) = &values[0] {
            assert!(map.get("timestamp").is_some());
            assert!(map.get("body").is_some());
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn test_decode_ndjson() {
        let body = Bytes::from(
            r#"{"time": 1703265600, "event": "line 1"}
{"time": 1703265601, "event": "line 2"}
{"time": 1703265602, "event": "line 3"}"#,
        );
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_decode_with_trailing_newline() {
        let body = Bytes::from(
            r#"{"event": "line 1"}
"#,
        );
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn test_decode_with_empty_lines() {
        let body = Bytes::from(
            r#"{"event": "line 1"}

{"event": "line 2"}
"#,
        );
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_decode_with_crlf() {
        let body = Bytes::from("{\"event\": \"line 1\"}\r\n{\"event\": \"line 2\"}\r\n");
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_decode_full_event() {
        let body = Bytes::from(
            r#"{"time": 1703265600.123, "host": "web-1", "source": "nginx", "sourcetype": "access", "event": "GET /health 200", "fields": {"env": "prod"}}"#,
        );
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 1);

        if let Value::Object(map) = &values[0] {
            assert_eq!(
                map.get("timestamp"),
                Some(&Value::Integer(1703265600123)) // milliseconds, preserving sub-second
            );
            assert_eq!(map.get("host"), Some(&Value::Bytes(Bytes::from("web-1"))));
            assert!(map.get("fields").is_some());
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn test_decode_object_event() {
        let body = Bytes::from(r#"{"event": {"message": "structured", "level": "info"}}"#);
        let values = decode_hec_logs(body).unwrap();
        assert_eq!(values.len(), 1);

        if let Value::Object(map) = &values[0] {
            // Object events are JSON-encoded as string
            if let Some(Value::Bytes(b)) = map.get("body") {
                let s = std::str::from_utf8(b).unwrap();
                assert!(s.contains("structured"));
            } else {
                panic!("expected bytes body");
            }
        }
    }

    // === Error case tests ===

    #[test]
    fn test_decode_invalid_json() {
        let body = Bytes::from(
            r#"{"event": "valid"}
{this is not valid json}
{"event": "also valid"}"#,
        );
        let result = decode_hec_logs(body);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("parse error"), "expected parse error: {}", err);
    }

    #[test]
    fn test_decode_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let result = decode_hec_logs(Bytes::from(invalid_utf8));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("UTF-8"), "expected UTF-8 error: {}", err);
    }

    #[test]
    fn test_decode_empty_body() {
        let result = decode_hec_logs(Bytes::from(""));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_decode_whitespace_only() {
        let result = decode_hec_logs(Bytes::from("   \n\n  \n"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_decode_missing_event_field() {
        // Missing event field results in null body
        let body = Bytes::from(r#"{"time": 123, "host": "test"}"#);
        let result = decode_hec_logs(body);
        assert!(result.is_ok());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);

        if let Value::Object(map) = &values[0] {
            assert_eq!(map.get("body"), Some(&Value::Null));
        }
    }

    #[test]
    fn test_epoch_to_millis_boundary() {
        // Near-millisecond boundary truncates (doesn't round)
        assert_eq!(safe_epoch_to_millis(0.0009).unwrap(), 0);
        assert_eq!(safe_epoch_to_millis(0.001).unwrap(), 1);
        assert_eq!(safe_epoch_to_millis(0.9999).unwrap(), 999);
        assert_eq!(safe_epoch_to_millis(1.0).unwrap(), 1000);
    }
}
