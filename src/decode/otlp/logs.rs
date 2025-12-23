use bytes::Bytes;
use vrl::value::Value as VrlValue;

use super::common::{looks_like_json, DecodeError, DecodeFormat};
use super::logs_json;
use super::logs_proto;

/// Decode OTLP logs (JSON or protobuf) into VRL Values, ready for transform.
pub fn decode_logs(body: Bytes, format: DecodeFormat) -> Result<Vec<VrlValue>, DecodeError> {
    match format {
        DecodeFormat::Json => logs_json::decode_json(&body),
        DecodeFormat::Protobuf => logs_proto::decode_protobuf(&body),
        DecodeFormat::Auto => {
            if looks_like_json(&body) {
                match logs_json::decode_json(&body) {
                    Ok(v) => Ok(v),
                    Err(json_err) => logs_proto::decode_protobuf(&body).map_err(|proto_err| {
                        DecodeError::Unsupported(format!(
                            "json decode failed: {}; protobuf fallback failed: {}",
                            json_err, proto_err
                        ))
                    }),
                }
            } else {
                match logs_proto::decode_protobuf(&body) {
                    Ok(v) => Ok(v),
                    Err(proto_err) => logs_json::decode_json(&body).map_err(|json_err| {
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
        collector::logs::v1::ExportLogsServiceRequest,
        common::v1::{any_value, AnyValue, InstrumentationScope, KeyValue},
        logs::v1::LogRecord,
        resource::v1::Resource,
    };
    use prost::Message;

    #[test]
    fn decodes_json_payload() {
        let body = r#"{
            "resourceLogs": [{
                "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": "svc" } }]},
                "scopeLogs": [{
                    "scope": { "name": "lib", "version": "1" },
                    "logRecords": [{
                        "timeUnixNano": "123",
                        "observedTimeUnixNano": "124",
                        "severityNumber": 9,
                        "severityText": "INFO",
                        "body": { "stringValue": "hello" },
                        "attributes": [{ "key": "k", "value": { "stringValue": "v" } }],
                        "traceId": "0af7651916cd43dd8448eb211c80319c",
                        "spanId": "b7ad6b7169203331"
                    }]
                }]
            }]
        }"#;

        let records = decode_logs(Bytes::from(body), DecodeFormat::Json).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn decodes_protobuf_payload() {
        let log = LogRecord {
            time_unix_nano: 123,
            observed_time_unix_nano: 124,
            severity_number: 9,
            severity_text: "INFO".to_string(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue("hello".to_string())),
            }),
            attributes: vec![KeyValue {
                key: "k".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue("v".to_string())),
                }),
            }],
            trace_id: vec![0, 1, 2, 3],
            span_id: vec![4, 5, 6, 7],
            ..Default::default()
        };

        let request = ExportLogsServiceRequest {
            resource_logs: vec![opentelemetry_proto::tonic::logs::v1::ResourceLogs {
                resource: Some(Resource {
                    attributes: vec![],
                    ..Default::default()
                }),
                scope_logs: vec![opentelemetry_proto::tonic::logs::v1::ScopeLogs {
                    scope: Some(InstrumentationScope {
                        name: "lib".to_string(),
                        version: "1".to_string(),
                        ..Default::default()
                    }),
                    log_records: vec![log],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let records = decode_logs(Bytes::from(body), DecodeFormat::Protobuf).unwrap();
        assert_eq!(records.len(), 1);
    }
}
