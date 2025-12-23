// JSON OTLP parsing for traces - struct definitions and conversion to VRL values

use bytes::Bytes;
use serde::Deserialize;
use std::sync::Arc;
use vrl::value::Value as VrlValue;

use super::common::{
    for_each_resource_scope, json_attrs_to_value, json_resource_to_value, json_scope_to_value,
    json_timestamp_to_i64, DecodeError, JsonInstrumentationScope, JsonKeyValue, JsonNumberOrString,
    JsonResource,
};
use crate::decode::record_builder::{
    build_span_record, preallocate_span_values, SpanEventParts, SpanLinkParts, SpanRecordParts,
};

pub fn decode_json(body: &Bytes) -> Result<Vec<VrlValue>, DecodeError> {
    let request: JsonExportTraceServiceRequest =
        serde_json::from_slice(body).map_err(DecodeError::Json)?;
    export_traces_json_to_vrl(request)
}

fn export_traces_json_to_vrl(
    request: JsonExportTraceServiceRequest,
) -> Result<Vec<VrlValue>, DecodeError> {
    let mut values = preallocate_span_values(&request.resource_spans, |rs| {
        rs.scope_spans.iter().map(|ss| ss.spans.len()).sum()
    });

    for_each_resource_scope(
        request.resource_spans,
        |resource_spans| {
            (
                json_resource_to_value(resource_spans.resource),
                resource_spans.scope_spans,
            )
        },
        |scope_spans| (json_scope_to_value(scope_spans.scope), scope_spans.spans),
        |spans, resource, scope| {
            for span in spans {
                let events: Vec<SpanEventParts> = span
                    .events
                    .into_iter()
                    .map(|e| {
                        Ok(SpanEventParts {
                            time_unix_nano: json_timestamp_to_i64(
                                &e.time_unix_nano,
                                "event.time_unix_nano",
                            )?,
                            name: Bytes::from(e.name),
                            attributes: json_attrs_to_value(e.attributes),
                        })
                    })
                    .collect::<Result<_, DecodeError>>()?;

                let links: Vec<SpanLinkParts> = span
                    .links
                    .into_iter()
                    .map(|l| SpanLinkParts {
                        trace_id: Bytes::from(l.trace_id),
                        span_id: Bytes::from(l.span_id),
                        trace_state: Bytes::from(l.trace_state),
                        attributes: json_attrs_to_value(l.attributes),
                    })
                    .collect();

                let parts = SpanRecordParts {
                    trace_id: Bytes::from(span.trace_id),
                    span_id: Bytes::from(span.span_id),
                    parent_span_id: Bytes::from(span.parent_span_id),
                    trace_state: Bytes::from(span.trace_state),
                    name: Bytes::from(span.name),
                    kind: span.kind as i64,
                    start_time_unix_nano: json_timestamp_to_i64(
                        &span.start_time_unix_nano,
                        "span.start_time_unix_nano",
                    )?,
                    end_time_unix_nano: json_timestamp_to_i64(
                        &span.end_time_unix_nano,
                        "span.end_time_unix_nano",
                    )?,
                    attributes: json_attrs_to_value(span.attributes),
                    status_code: span.status.code as i64,
                    status_message: Bytes::from(span.status.message),
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

// JSON struct definitions for OTLP traces

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonExportTraceServiceRequest {
    #[serde(default)]
    resource_spans: Vec<JsonResourceSpans>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonResourceSpans {
    #[serde(default)]
    resource: JsonResource,
    #[serde(default)]
    scope_spans: Vec<JsonScopeSpans>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonScopeSpans {
    #[serde(default)]
    scope: JsonInstrumentationScope,
    #[serde(default)]
    spans: Vec<JsonSpan>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonSpan {
    #[serde(default)]
    trace_id: String,
    #[serde(default)]
    span_id: String,
    #[serde(default)]
    parent_span_id: String,
    #[serde(default)]
    trace_state: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    kind: i32,
    #[serde(default)]
    start_time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    end_time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    attributes: Vec<JsonKeyValue>,
    #[serde(default)]
    events: Vec<JsonSpanEvent>,
    #[serde(default)]
    links: Vec<JsonSpanLink>,
    #[serde(default)]
    status: JsonStatus,
    #[serde(default)]
    dropped_attributes_count: u32,
    #[serde(default)]
    dropped_events_count: u32,
    #[serde(default)]
    dropped_links_count: u32,
    #[serde(default)]
    flags: u32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonSpanEvent {
    #[serde(default)]
    time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    name: String,
    #[serde(default)]
    attributes: Vec<JsonKeyValue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonSpanLink {
    #[serde(default)]
    trace_id: String,
    #[serde(default)]
    span_id: String,
    #[serde(default)]
    trace_state: String,
    #[serde(default)]
    attributes: Vec<JsonKeyValue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonStatus {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    message: String,
}
