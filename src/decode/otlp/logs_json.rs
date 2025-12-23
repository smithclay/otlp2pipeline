// JSON OTLP parsing for logs - struct definitions and conversion to VRL values

use bytes::Bytes;
use serde::Deserialize;
use std::sync::Arc;
use vrl::value::Value as VrlValue;

use super::common::{
    for_each_resource_scope, json_any_value_to_vrl, json_attrs_to_value, json_resource_to_value,
    json_scope_to_value, json_timestamp_to_i64, DecodeError, JsonAnyValue,
    JsonInstrumentationScope, JsonKeyValue, JsonNumberOrString, JsonResource,
};
use crate::decode::record_builder::{build_log_record, preallocate_log_values, LogRecordParts};

pub fn decode_json(body: &Bytes) -> Result<Vec<VrlValue>, DecodeError> {
    let request: JsonExportLogsServiceRequest =
        serde_json::from_slice(body).map_err(DecodeError::Json)?;
    export_logs_json_to_vrl(request)
}

fn export_logs_json_to_vrl(
    request: JsonExportLogsServiceRequest,
) -> Result<Vec<VrlValue>, DecodeError> {
    let mut values = preallocate_log_values(&request.resource_logs, |rl| {
        rl.scope_logs.iter().map(|sl| sl.log_records.len()).sum()
    });

    for_each_resource_scope(
        request.resource_logs,
        |resource_logs| {
            (
                json_resource_to_value(resource_logs.resource),
                resource_logs.scope_logs,
            )
        },
        |scope_logs| {
            (
                json_scope_to_value(scope_logs.scope),
                scope_logs.log_records,
            )
        },
        |log_records, resource, scope| {
            for log_record in log_records {
                let body = log_record
                    .body
                    .map(json_any_value_to_vrl)
                    .unwrap_or(VrlValue::Null);

                let parts = LogRecordParts {
                    time_unix_nano: json_timestamp_to_i64(
                        &log_record.time_unix_nano,
                        "log.time_unix_nano",
                    )?,
                    observed_time_unix_nano: json_timestamp_to_i64(
                        &log_record.observed_time_unix_nano,
                        "log.observed_time_unix_nano",
                    )?,
                    severity_number: log_record.severity_number as i64,
                    severity_text: Bytes::from(log_record.severity_text),
                    body,
                    trace_id: Bytes::from(log_record.trace_id),
                    span_id: Bytes::from(log_record.span_id),
                    attributes: json_attrs_to_value(log_record.attributes),
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

// JSON struct definitions for OTLP logs

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonExportLogsServiceRequest {
    #[serde(default)]
    resource_logs: Vec<JsonResourceLogs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonResourceLogs {
    #[serde(default)]
    resource: JsonResource,
    #[serde(default)]
    scope_logs: Vec<JsonScopeLogs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonScopeLogs {
    #[serde(default)]
    scope: JsonInstrumentationScope,
    #[serde(default)]
    log_records: Vec<JsonLogRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonLogRecord {
    #[serde(default)]
    time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    observed_time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    severity_number: i32,
    #[serde(default)]
    severity_text: String,
    #[serde(default)]
    body: Option<JsonAnyValue>,
    #[serde(default)]
    attributes: Vec<JsonKeyValue>,
    #[serde(default)]
    trace_id: String,
    #[serde(default)]
    span_id: String,
}
