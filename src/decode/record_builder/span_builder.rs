use bytes::Bytes;
use std::sync::Arc;
use vrl::value::{ObjectMap, Value as VrlValue};

/// Precomputed fields for building a span record into VRL values
pub struct SpanRecordParts {
    pub trace_id: Bytes,
    pub span_id: Bytes,
    pub parent_span_id: Bytes,
    pub trace_state: Bytes,
    pub name: Bytes,
    pub kind: i64,
    pub start_time_unix_nano: i64,
    pub end_time_unix_nano: i64,
    pub attributes: VrlValue,
    pub status_code: i64,
    pub status_message: Bytes,
    pub events: Vec<SpanEventParts>,
    pub links: Vec<SpanLinkParts>,
    pub resource: Arc<VrlValue>,
    pub scope: Arc<VrlValue>,
    pub dropped_attributes_count: i64,
    pub dropped_events_count: i64,
    pub dropped_links_count: i64,
    pub flags: i64,
}

pub struct SpanEventParts {
    pub time_unix_nano: i64,
    pub name: Bytes,
    pub attributes: VrlValue,
}

pub struct SpanLinkParts {
    pub trace_id: Bytes,
    pub span_id: Bytes,
    pub trace_state: Bytes,
    pub attributes: VrlValue,
}

/// Pre-allocate a values Vec sized to the number of spans a request contains
pub fn preallocate_span_values<R, F>(resource_spans: &[R], count_spans: F) -> Vec<VrlValue>
where
    F: Fn(&R) -> usize,
{
    let capacity: usize = resource_spans.iter().map(&count_spans).sum();
    Vec::with_capacity(capacity)
}

/// Build a VRL-ready span record from parts
pub fn build_span_record(parts: SpanRecordParts) -> VrlValue {
    // Debug assertions to catch schema violations early
    debug_assert!(
        parts.start_time_unix_nano >= 0,
        "span start timestamp must be non-negative"
    );
    debug_assert!(
        parts.end_time_unix_nano >= 0,
        "span end timestamp must be non-negative"
    );
    debug_assert!(
        (0..=5).contains(&parts.kind),
        "span kind must be 0-5, got: {}",
        parts.kind
    );
    debug_assert!(
        (0..=2).contains(&parts.status_code),
        "status_code must be 0-2, got: {}",
        parts.status_code
    );
    debug_assert!(!parts.trace_id.is_empty(), "trace_id must not be empty");
    debug_assert!(!parts.span_id.is_empty(), "span_id must not be empty");

    let mut map = ObjectMap::new();

    // Basic span identifiers
    map.insert("trace_id".into(), VrlValue::Bytes(parts.trace_id));
    map.insert("span_id".into(), VrlValue::Bytes(parts.span_id));
    map.insert(
        "parent_span_id".into(),
        VrlValue::Bytes(parts.parent_span_id),
    );
    map.insert("trace_state".into(), VrlValue::Bytes(parts.trace_state));
    map.insert("name".into(), VrlValue::Bytes(parts.name));
    map.insert("kind".into(), VrlValue::Integer(parts.kind));

    // Timestamps and duration
    map.insert(
        "start_time_unix_nano".into(),
        VrlValue::Integer(parts.start_time_unix_nano),
    );
    map.insert(
        "end_time_unix_nano".into(),
        VrlValue::Integer(parts.end_time_unix_nano),
    );
    map.insert(
        "duration_ns".into(),
        VrlValue::Integer(
            parts
                .end_time_unix_nano
                .saturating_sub(parts.start_time_unix_nano),
        ),
    );

    // Attributes
    map.insert("attributes".into(), parts.attributes);

    // Status
    map.insert("status_code".into(), VrlValue::Integer(parts.status_code));
    map.insert(
        "status_message".into(),
        VrlValue::Bytes(parts.status_message),
    );

    // Events
    let events_array: Vec<VrlValue> = parts
        .events
        .into_iter()
        .map(|e| {
            let mut event_map = ObjectMap::new();
            event_map.insert("time_unix_nano".into(), VrlValue::Integer(e.time_unix_nano));
            event_map.insert("name".into(), VrlValue::Bytes(e.name));
            event_map.insert("attributes".into(), e.attributes);
            VrlValue::Object(event_map)
        })
        .collect();
    map.insert("events".into(), VrlValue::Array(events_array));

    // Links
    let links_array: Vec<VrlValue> = parts
        .links
        .into_iter()
        .map(|l| {
            let mut link_map = ObjectMap::new();
            link_map.insert("trace_id".into(), VrlValue::Bytes(l.trace_id));
            link_map.insert("span_id".into(), VrlValue::Bytes(l.span_id));
            link_map.insert("trace_state".into(), VrlValue::Bytes(l.trace_state));
            link_map.insert("attributes".into(), l.attributes);
            VrlValue::Object(link_map)
        })
        .collect();
    map.insert("links".into(), VrlValue::Array(links_array));

    // Resource and scope
    map.insert("resource".into(), (*parts.resource).clone());
    map.insert("scope".into(), (*parts.scope).clone());

    // Dropped counts and flags
    map.insert(
        "dropped_attributes_count".into(),
        VrlValue::Integer(parts.dropped_attributes_count),
    );
    map.insert(
        "dropped_events_count".into(),
        VrlValue::Integer(parts.dropped_events_count),
    );
    map.insert(
        "dropped_links_count".into(),
        VrlValue::Integer(parts.dropped_links_count),
    );
    map.insert("flags".into(), VrlValue::Integer(parts.flags));

    VrlValue::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_span_record_with_dropped_counts_and_flags() {
        let parts = SpanRecordParts {
            trace_id: Bytes::from("abc123"),
            span_id: Bytes::from("def456"),
            parent_span_id: Bytes::from(""),
            trace_state: Bytes::from(""),
            name: Bytes::from("test-span"),
            kind: 2, // SERVER
            start_time_unix_nano: 1000,
            end_time_unix_nano: 2000,
            attributes: VrlValue::Object(ObjectMap::new()),
            status_code: 1,
            status_message: Bytes::from("OK"),
            events: vec![],
            links: vec![],
            resource: Arc::new(VrlValue::Object(ObjectMap::new())),
            scope: Arc::new(VrlValue::Object(ObjectMap::new())),
            dropped_attributes_count: 5,
            dropped_events_count: 3,
            dropped_links_count: 2,
            flags: 1,
        };

        let record = build_span_record(parts);

        let obj = match record {
            VrlValue::Object(map) => map,
            _ => panic!("expected object"),
        };

        assert_eq!(
            obj.get("dropped_attributes_count"),
            Some(&VrlValue::Integer(5))
        );
        assert_eq!(obj.get("dropped_events_count"), Some(&VrlValue::Integer(3)));
        assert_eq!(obj.get("dropped_links_count"), Some(&VrlValue::Integer(2)));
        assert_eq!(obj.get("flags"), Some(&VrlValue::Integer(1)));
        assert_eq!(obj.get("kind"), Some(&VrlValue::Integer(2)));
        assert_eq!(
            obj.get("name"),
            Some(&VrlValue::Bytes(Bytes::from("test-span")))
        );
        assert_eq!(obj.get("duration_ns"), Some(&VrlValue::Integer(1000)));
    }
}
