use bytes::Bytes;
use const_hex::encode as hex_encode;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use prost::Message;
use std::sync::Arc;
use vrl::value::Value as VrlValue;

use super::common::{
    for_each_resource_scope, otlp_any_value_to_vrl, otlp_attributes_to_value,
    otlp_resource_to_value, otlp_scope_to_value, safe_timestamp_conversion, DecodeError,
};
use crate::decode::record_builder::{build_log_record, preallocate_log_values, LogRecordParts};

pub fn decode_protobuf(body: &Bytes) -> Result<Vec<VrlValue>, DecodeError> {
    let request = ExportLogsServiceRequest::decode(body.as_ref()).map_err(DecodeError::Protobuf)?;
    export_logs_to_vrl_proto(request)
}

fn export_logs_to_vrl_proto(
    request: ExportLogsServiceRequest,
) -> Result<Vec<VrlValue>, DecodeError> {
    let mut values = preallocate_log_values(&request.resource_logs, |rl| {
        rl.scope_logs.iter().map(|sl| sl.log_records.len()).sum()
    });

    for_each_resource_scope(
        request.resource_logs,
        |resource_logs| {
            (
                otlp_resource_to_value(resource_logs.resource.as_ref()),
                resource_logs.scope_logs,
            )
        },
        |scope_logs| {
            (
                otlp_scope_to_value(scope_logs.scope.as_ref()),
                scope_logs.log_records,
            )
        },
        |log_records, resource, scope| {
            for log_record in log_records {
                let body = log_record
                    .body
                    .as_ref()
                    .map(otlp_any_value_to_vrl)
                    .unwrap_or(VrlValue::Null);

                let parts = LogRecordParts {
                    time_unix_nano: safe_timestamp_conversion(
                        log_record.time_unix_nano,
                        "log.time_unix_nano",
                    )?,
                    observed_time_unix_nano: safe_timestamp_conversion(
                        log_record.observed_time_unix_nano,
                        "log.observed_time_unix_nano",
                    )?,
                    severity_number: log_record.severity_number as i64,
                    severity_text: Bytes::from(log_record.severity_text),
                    body,
                    trace_id: Bytes::from(hex_encode(&log_record.trace_id)),
                    span_id: Bytes::from(hex_encode(&log_record.span_id)),
                    attributes: otlp_attributes_to_value(&log_record.attributes),
                    resource: Arc::clone(&resource),
                    scope: Arc::clone(&scope),
                };

                values.push(build_log_record(parts));
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
        common::v1::{any_value, AnyValue, InstrumentationScope, KeyValue},
        logs::v1::LogRecord,
        resource::v1::Resource,
    };

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
        let records = decode_protobuf(&Bytes::from(body)).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn rejects_overflow_log_time() {
        let log = LogRecord {
            time_unix_nano: u64::MAX, // Overflow value
            observed_time_unix_nano: 124,
            severity_number: 9,
            severity_text: "INFO".to_string(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue("hello".to_string())),
            }),
            ..Default::default()
        };

        let request = ExportLogsServiceRequest {
            resource_logs: vec![opentelemetry_proto::tonic::logs::v1::ResourceLogs {
                resource: Some(Resource::default()),
                scope_logs: vec![opentelemetry_proto::tonic::logs::v1::ScopeLogs {
                    scope: Some(InstrumentationScope::default()),
                    log_records: vec![log],
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
                assert!(msg.contains("time_unix_nano"));
            }
            _ => panic!("Expected DecodeError::Unsupported, got: {:?}", err),
        }
    }

    #[test]
    fn rejects_overflow_log_observed_time() {
        let log = LogRecord {
            time_unix_nano: 123,
            observed_time_unix_nano: u64::MAX, // Overflow value
            severity_number: 9,
            severity_text: "INFO".to_string(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue("hello".to_string())),
            }),
            ..Default::default()
        };

        let request = ExportLogsServiceRequest {
            resource_logs: vec![opentelemetry_proto::tonic::logs::v1::ResourceLogs {
                resource: Some(Resource::default()),
                scope_logs: vec![opentelemetry_proto::tonic::logs::v1::ScopeLogs {
                    scope: Some(InstrumentationScope::default()),
                    log_records: vec![log],
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
                assert!(msg.contains("observed_time_unix_nano"));
            }
            _ => panic!("Expected DecodeError::Unsupported, got: {:?}", err),
        }
    }

    #[test]
    fn accepts_valid_log_timestamps() {
        let log = LogRecord {
            time_unix_nano: 123,
            observed_time_unix_nano: 124,
            severity_number: 9,
            severity_text: "INFO".to_string(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue("hello".to_string())),
            }),
            ..Default::default()
        };

        let request = ExportLogsServiceRequest {
            resource_logs: vec![opentelemetry_proto::tonic::logs::v1::ResourceLogs {
                resource: Some(Resource::default()),
                scope_logs: vec![opentelemetry_proto::tonic::logs::v1::ScopeLogs {
                    scope: Some(InstrumentationScope::default()),
                    log_records: vec![log],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body));

        assert!(result.is_ok());
        let logs = result.unwrap();
        assert_eq!(logs.len(), 1);
    }
}
