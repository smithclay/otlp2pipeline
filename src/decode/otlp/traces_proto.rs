use bytes::Bytes;
use const_hex::encode as hex_encode;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use prost::Message;
use std::sync::Arc;
use vrl::value::Value as VrlValue;

use super::common::{
    for_each_resource_scope, otlp_attributes_to_value, otlp_resource_to_value, otlp_scope_to_value,
    safe_timestamp_conversion, DecodeError,
};
use crate::decode::record_builder::{
    build_span_record, preallocate_span_values, SpanEventParts, SpanLinkParts, SpanRecordParts,
};

pub fn decode_protobuf(body: &Bytes) -> Result<Vec<VrlValue>, DecodeError> {
    let request =
        ExportTraceServiceRequest::decode(body.as_ref()).map_err(DecodeError::Protobuf)?;
    export_traces_to_vrl_proto(request)
}

fn export_traces_to_vrl_proto(
    request: ExportTraceServiceRequest,
) -> Result<Vec<VrlValue>, DecodeError> {
    let mut values = preallocate_span_values(&request.resource_spans, |rs| {
        rs.scope_spans.iter().map(|ss| ss.spans.len()).sum()
    });

    for_each_resource_scope(
        request.resource_spans,
        |resource_spans| {
            (
                otlp_resource_to_value(resource_spans.resource.as_ref()),
                resource_spans.scope_spans,
            )
        },
        |scope_spans| {
            (
                otlp_scope_to_value(scope_spans.scope.as_ref()),
                scope_spans.spans,
            )
        },
        |spans, resource, scope| {
            for span in spans {
                let events: Result<Vec<SpanEventParts>, DecodeError> = span
                    .events
                    .iter()
                    .map(|e| {
                        Ok(SpanEventParts {
                            time_unix_nano: safe_timestamp_conversion(
                                e.time_unix_nano,
                                "event.time_unix_nano",
                            )?,
                            name: Bytes::from(e.name.clone()),
                            attributes: otlp_attributes_to_value(&e.attributes),
                        })
                    })
                    .collect();
                let events = events?;

                let links: Vec<SpanLinkParts> = span
                    .links
                    .iter()
                    .map(|l| SpanLinkParts {
                        trace_id: Bytes::from(hex_encode(&l.trace_id)),
                        span_id: Bytes::from(hex_encode(&l.span_id)),
                        trace_state: Bytes::from(l.trace_state.clone()),
                        attributes: otlp_attributes_to_value(&l.attributes),
                    })
                    .collect();

                let (status_code, status_message) = span
                    .status
                    .as_ref()
                    .map(|s| (s.code as i64, Bytes::from(s.message.clone())))
                    .unwrap_or((0, Bytes::new()));

                let parts = SpanRecordParts {
                    trace_id: Bytes::from(hex_encode(&span.trace_id)),
                    span_id: Bytes::from(hex_encode(&span.span_id)),
                    parent_span_id: Bytes::from(hex_encode(&span.parent_span_id)),
                    trace_state: Bytes::from(span.trace_state),
                    name: Bytes::from(span.name),
                    kind: span.kind as i64,
                    start_time_unix_nano: safe_timestamp_conversion(
                        span.start_time_unix_nano,
                        "span.start_time_unix_nano",
                    )?,
                    end_time_unix_nano: safe_timestamp_conversion(
                        span.end_time_unix_nano,
                        "span.end_time_unix_nano",
                    )?,
                    attributes: otlp_attributes_to_value(&span.attributes),
                    status_code,
                    status_message,
                    events,
                    links,
                    resource: Arc::clone(&resource),
                    scope: Arc::clone(&scope),
                    dropped_attributes_count: span.dropped_attributes_count as i64,
                    dropped_events_count: span.dropped_events_count as i64,
                    dropped_links_count: span.dropped_links_count as i64,
                    flags: span.flags as i64,
                };

                values.push(build_span_record(parts));
            }

            Ok(())
        },
    )?;

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::{
        collector::trace::v1::ExportTraceServiceRequest,
        common::v1::InstrumentationScope,
        resource::v1::Resource,
        trace::v1::{span, ResourceSpans, ScopeSpans, Span},
    };

    #[test]
    fn rejects_overflow_span_start_time() {
        let span = Span {
            trace_id: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            span_id: vec![0, 1, 2, 3, 4, 5, 6, 7],
            name: "test-span".to_string(),
            start_time_unix_nano: u64::MAX, // Overflow value
            end_time_unix_nano: 1_000_000_000,
            ..Default::default()
        };

        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource::default()),
                scope_spans: vec![ScopeSpans {
                    scope: Some(InstrumentationScope::default()),
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body));

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            DecodeError::Unsupported(msg) => {
                assert!(msg.contains("timestamp overflow"));
                assert!(msg.contains("start_time_unix_nano"));
            }
            _ => panic!("Expected DecodeError::Unsupported, got: {:?}", err),
        }
    }

    #[test]
    fn rejects_overflow_span_end_time() {
        let span = Span {
            trace_id: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            span_id: vec![0, 1, 2, 3, 4, 5, 6, 7],
            name: "test-span".to_string(),
            start_time_unix_nano: 1_000_000_000,
            end_time_unix_nano: u64::MAX, // Overflow value
            ..Default::default()
        };

        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource::default()),
                scope_spans: vec![ScopeSpans {
                    scope: Some(InstrumentationScope::default()),
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body));

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            DecodeError::Unsupported(msg) => {
                assert!(msg.contains("timestamp overflow"));
                assert!(msg.contains("end_time_unix_nano"));
            }
            _ => panic!("Expected DecodeError::Unsupported, got: {:?}", err),
        }
    }

    #[test]
    fn rejects_overflow_event_time() {
        let span = Span {
            trace_id: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            span_id: vec![0, 1, 2, 3, 4, 5, 6, 7],
            name: "test-span".to_string(),
            start_time_unix_nano: 1_000_000_000,
            end_time_unix_nano: 2_000_000_000,
            events: vec![span::Event {
                time_unix_nano: u64::MAX, // Overflow value
                name: "test-event".to_string(),
                attributes: vec![],
                ..Default::default()
            }],
            ..Default::default()
        };

        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource::default()),
                scope_spans: vec![ScopeSpans {
                    scope: Some(InstrumentationScope::default()),
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body));

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            DecodeError::Unsupported(msg) => {
                assert!(msg.contains("timestamp overflow"));
                assert!(msg.contains("event.time_unix_nano"));
            }
            _ => panic!("Expected DecodeError::Unsupported, got: {:?}", err),
        }
    }

    #[test]
    fn accepts_valid_timestamps() {
        let span = Span {
            trace_id: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            span_id: vec![0, 1, 2, 3, 4, 5, 6, 7],
            name: "test-span".to_string(),
            start_time_unix_nano: 1_000_000_000,
            end_time_unix_nano: 2_000_000_000,
            events: vec![span::Event {
                time_unix_nano: 1_500_000_000,
                name: "test-event".to_string(),
                attributes: vec![],
                ..Default::default()
            }],
            ..Default::default()
        };

        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource::default()),
                scope_spans: vec![ScopeSpans {
                    scope: Some(InstrumentationScope::default()),
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body));

        assert!(result.is_ok());
        let spans = result.unwrap();
        assert_eq!(spans.len(), 1);
    }
}
