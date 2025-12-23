use bytes::Bytes;
use vrl::value::Value as VrlValue;

use super::common::{looks_like_json, DecodeError, DecodeFormat};
use super::traces_json;
use super::traces_proto;

/// Decode OTLP traces (JSON or protobuf) into VRL Values, ready for transform.
pub fn decode_traces(body: Bytes, format: DecodeFormat) -> Result<Vec<VrlValue>, DecodeError> {
    match format {
        DecodeFormat::Json => traces_json::decode_json(&body),
        DecodeFormat::Protobuf => traces_proto::decode_protobuf(&body),
        DecodeFormat::Auto => {
            if looks_like_json(&body) {
                match traces_json::decode_json(&body) {
                    Ok(v) => Ok(v),
                    Err(json_err) => traces_proto::decode_protobuf(&body).map_err(|proto_err| {
                        DecodeError::Unsupported(format!(
                            "json decode failed: {}; protobuf fallback failed: {}",
                            json_err, proto_err
                        ))
                    }),
                }
            } else {
                match traces_proto::decode_protobuf(&body) {
                    Ok(v) => Ok(v),
                    Err(proto_err) => traces_json::decode_json(&body).map_err(|json_err| {
                        DecodeError::Unsupported(format!(
                            "protobuf decode failed: {}; json fallback failed: {}",
                            proto_err, json_err
                        ))
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::{
        collector::trace::v1::ExportTraceServiceRequest,
        common::v1::{any_value, AnyValue, InstrumentationScope, KeyValue},
        resource::v1::Resource,
        trace::v1::{span, ResourceSpans, ScopeSpans, Span, Status},
    };
    use prost::Message;

    #[test]
    fn decodes_json_payload() {
        let body = r#"{
            "resourceSpans": [{
                "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": "svc" } }]},
                "scopeSpans": [{
                    "scope": { "name": "lib", "version": "1" },
                    "spans": [{
                        "traceId": "0af7651916cd43dd8448eb211c80319c",
                        "spanId": "b7ad6b7169203331",
                        "parentSpanId": "",
                        "name": "test-span",
                        "kind": 2,
                        "startTimeUnixNano": "1000000000",
                        "endTimeUnixNano": "2000000000",
                        "attributes": [{ "key": "k", "value": { "stringValue": "v" } }],
                        "status": { "code": 1, "message": "OK" }
                    }]
                }]
            }]
        }"#;

        let records = decode_traces(Bytes::from(body), DecodeFormat::Json).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn decodes_protobuf_payload() {
        let span = Span {
            trace_id: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            span_id: vec![0, 1, 2, 3, 4, 5, 6, 7],
            parent_span_id: vec![],
            name: "test-span".to_string(),
            kind: span::SpanKind::Server as i32,
            start_time_unix_nano: 1_000_000_000,
            end_time_unix_nano: 2_000_000_000,
            attributes: vec![KeyValue {
                key: "k".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue("v".to_string())),
                }),
            }],
            status: Some(Status {
                code: 1,
                message: "OK".to_string(),
            }),
            ..Default::default()
        };

        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![],
                    ..Default::default()
                }),
                scope_spans: vec![ScopeSpans {
                    scope: Some(InstrumentationScope {
                        name: "lib".to_string(),
                        version: "1".to_string(),
                        ..Default::default()
                    }),
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let records = decode_traces(Bytes::from(body), DecodeFormat::Protobuf).unwrap();
        assert_eq!(records.len(), 1);
    }
}
