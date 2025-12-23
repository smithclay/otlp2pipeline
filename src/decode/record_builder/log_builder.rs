use bytes::Bytes;
use std::sync::Arc;
use vrl::value::{ObjectMap, Value as VrlValue};

/// Precomputed fields for building a log record into VRL values
pub struct LogRecordParts {
    pub time_unix_nano: i64,
    pub observed_time_unix_nano: i64,
    pub severity_number: i64,
    pub severity_text: Bytes,
    pub body: VrlValue,
    pub trace_id: Bytes,
    pub span_id: Bytes,
    pub attributes: VrlValue,
    pub resource: Arc<VrlValue>,
    pub scope: Arc<VrlValue>,
}

/// Pre-allocate a values Vec sized to the number of log records a request contains
pub fn preallocate_log_values<R, F>(resource_logs: &[R], count_logs: F) -> Vec<VrlValue>
where
    F: Fn(&R) -> usize,
{
    let capacity: usize = resource_logs.iter().map(&count_logs).sum();
    Vec::with_capacity(capacity)
}

/// Build a VRL-ready log record from parts
pub fn build_log_record(parts: LogRecordParts) -> VrlValue {
    // Debug assertions to catch schema violations early
    debug_assert!(
        parts.time_unix_nano >= 0,
        "log timestamp must be non-negative"
    );
    debug_assert!(
        parts.observed_time_unix_nano >= 0,
        "log observed timestamp must be non-negative"
    );
    debug_assert!(
        (0..=24).contains(&parts.severity_number),
        "severity_number must be 0-24, got: {}",
        parts.severity_number
    );

    let mut map = ObjectMap::new();
    map.insert(
        "time_unix_nano".into(),
        VrlValue::Integer(parts.time_unix_nano),
    );
    map.insert(
        "observed_time_unix_nano".into(),
        VrlValue::Integer(parts.observed_time_unix_nano),
    );
    map.insert(
        "severity_number".into(),
        VrlValue::Integer(parts.severity_number),
    );
    map.insert("severity_text".into(), VrlValue::Bytes(parts.severity_text));
    map.insert("body".into(), parts.body);
    map.insert("trace_id".into(), VrlValue::Bytes(parts.trace_id));
    map.insert("span_id".into(), VrlValue::Bytes(parts.span_id));
    map.insert("attributes".into(), parts.attributes);
    map.insert("resource".into(), (*parts.resource).clone());
    map.insert("scope".into(), (*parts.scope).clone());
    VrlValue::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_complete_log_record() {
        let parts = LogRecordParts {
            time_unix_nano: 123,
            observed_time_unix_nano: 124,
            severity_number: 9,
            severity_text: Bytes::from("INFO"),
            body: VrlValue::Bytes(Bytes::from("test message")),
            trace_id: Bytes::from("abc123"),
            span_id: Bytes::from("def456"),
            attributes: VrlValue::Object(ObjectMap::new()),
            resource: Arc::new(VrlValue::Object(ObjectMap::new())),
            scope: Arc::new(VrlValue::Object(ObjectMap::new())),
        };

        let record = build_log_record(parts);
        let obj = match record {
            VrlValue::Object(map) => map,
            _ => panic!("expected object"),
        };

        assert_eq!(obj.get("time_unix_nano"), Some(&VrlValue::Integer(123)));
        assert_eq!(obj.get("severity_number"), Some(&VrlValue::Integer(9)));
        assert_eq!(
            obj.get("severity_text"),
            Some(&VrlValue::Bytes(Bytes::from("INFO")))
        );
    }
}
