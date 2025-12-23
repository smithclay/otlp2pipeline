use bytes::Bytes;
use serde::Deserialize;
use std::sync::Arc;
use tracing::debug;
use vrl::value::Value as VrlValue;

use super::common::{
    decode_bytes_field, finite_float_to_vrl, for_each_resource_scope, json_attrs_to_value,
    json_resource_to_value, json_scope_to_value, json_timestamp_to_i64, DecodeError,
    JsonInstrumentationScope, JsonKeyValue, JsonNumberOrString, JsonResource,
};
use crate::decode::record_builder::{
    build_gauge_record, build_sum_record, preallocate_metric_values, ExemplarParts,
    GaugeRecordParts, SumRecordParts,
};

/// Shared context for metric metadata to reduce function argument count
struct MetricContext {
    metric_name: Bytes,
    metric_description: Bytes,
    metric_unit: Bytes,
    resource: Arc<VrlValue>,
    scope: Arc<VrlValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonExportMetricsServiceRequest {
    #[serde(default)]
    resource_metrics: Vec<JsonResourceMetrics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonResourceMetrics {
    #[serde(default)]
    resource: JsonResource,
    #[serde(default)]
    scope_metrics: Vec<JsonScopeMetrics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonScopeMetrics {
    #[serde(default)]
    scope: JsonInstrumentationScope,
    #[serde(default)]
    metrics: Vec<JsonMetric>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonMetric {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    gauge: Option<JsonGauge>,
    #[serde(default)]
    sum: Option<JsonSum>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonGauge {
    #[serde(default)]
    data_points: Vec<JsonNumberDataPoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonSum {
    #[serde(default)]
    data_points: Vec<JsonNumberDataPoint>,
    #[serde(default)]
    aggregation_temporality: i64,
    #[serde(default)]
    is_monotonic: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonNumberDataPoint {
    #[serde(default)]
    time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    start_time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    as_int: Option<JsonNumberOrString>,
    #[serde(default)]
    as_double: Option<f64>,
    #[serde(default)]
    attributes: Vec<JsonKeyValue>,
    #[serde(default)]
    flags: u32,
    #[serde(default)]
    exemplars: Vec<JsonExemplar>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonExemplar {
    #[serde(default)]
    time_unix_nano: JsonNumberOrString,
    #[serde(default)]
    as_int: Option<JsonNumberOrString>,
    #[serde(default)]
    as_double: Option<f64>,
    #[serde(default)]
    trace_id: String,
    #[serde(default)]
    span_id: String,
    #[serde(default)]
    filtered_attributes: Vec<JsonKeyValue>,
}

pub fn decode_json(body: &Bytes) -> Result<Vec<VrlValue>, DecodeError> {
    let request: JsonExportMetricsServiceRequest =
        serde_json::from_slice(body).map_err(DecodeError::Json)?;
    export_metrics_to_vrl_json(request)
}

fn export_metrics_to_vrl_json(
    request: JsonExportMetricsServiceRequest,
) -> Result<Vec<VrlValue>, DecodeError> {
    let mut values = preallocate_metric_values(&request.resource_metrics, |rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .map(|m| {
                m.gauge.as_ref().map(|g| g.data_points.len()).unwrap_or(0)
                    + m.sum.as_ref().map(|s| s.data_points.len()).unwrap_or(0)
            })
            .sum()
    });

    for_each_resource_scope(
        request.resource_metrics,
        |resource_metrics| {
            (
                json_resource_to_value(resource_metrics.resource),
                resource_metrics.scope_metrics,
            )
        },
        |scope_metrics| {
            (
                json_scope_to_value(scope_metrics.scope),
                scope_metrics.metrics,
            )
        },
        |metrics, resource, scope| {
            for metric in metrics {
                let mut emitted = false;
                let ctx = MetricContext {
                    metric_name: Bytes::from(metric.name),
                    metric_description: Bytes::from(metric.description),
                    metric_unit: Bytes::from(metric.unit),
                    resource: Arc::clone(&resource),
                    scope: Arc::clone(&scope),
                };

                if let Some(gauge) = metric.gauge {
                    for point in gauge.data_points {
                        if let Some(record) = build_gauge_from_json_point(point, &ctx)? {
                            values.push(record);
                        }
                    }
                    emitted = true;
                }

                if let Some(sum) = metric.sum {
                    for point in sum.data_points {
                        if let Some(record) = build_sum_from_json_point(
                            point,
                            &ctx,
                            sum.aggregation_temporality,
                            sum.is_monotonic,
                        )? {
                            values.push(record);
                        }
                    }
                    emitted = true;
                }

                if !emitted {
                    debug!(
                        metric_name = %ctx.metric_name.escape_ascii(),
                        "skipping metric with unsupported type (expected gauge or sum)"
                    );
                }
            }

            Ok(())
        },
    )?;

    Ok(values)
}

fn build_gauge_from_json_point(
    point: JsonNumberDataPoint,
    ctx: &MetricContext,
) -> Result<Option<VrlValue>, DecodeError> {
    // Skip records with missing or non-finite values (NaN/Infinity)
    let value = match extract_number_value(&point.as_int, &point.as_double) {
        Some(v) => v,
        None => {
            debug!(metric_name = %String::from_utf8_lossy(&ctx.metric_name), "skipping gauge point with missing or non-finite value");
            return Ok(None);
        }
    };

    let exemplars = build_json_exemplars(point.exemplars)?;

    let parts = GaugeRecordParts {
        time_unix_nano: json_timestamp_to_i64(&point.time_unix_nano, "gauge.time_unix_nano")?,
        start_time_unix_nano: json_timestamp_to_i64(
            &point.start_time_unix_nano,
            "gauge.start_time_unix_nano",
        )?,
        metric_name: ctx.metric_name.clone(),
        metric_description: ctx.metric_description.clone(),
        metric_unit: ctx.metric_unit.clone(),
        value,
        attributes: json_attrs_to_value(point.attributes),
        resource: Arc::clone(&ctx.resource),
        scope: Arc::clone(&ctx.scope),
        flags: point.flags as i64,
        exemplars,
    };

    Ok(Some(build_gauge_record(parts)))
}

fn build_sum_from_json_point(
    point: JsonNumberDataPoint,
    ctx: &MetricContext,
    aggregation_temporality: i64,
    is_monotonic: bool,
) -> Result<Option<VrlValue>, DecodeError> {
    // Skip records with missing or non-finite values (NaN/Infinity)
    let value = match extract_number_value(&point.as_int, &point.as_double) {
        Some(v) => v,
        None => {
            debug!(metric_name = %String::from_utf8_lossy(&ctx.metric_name), "skipping sum point with missing or non-finite value");
            return Ok(None);
        }
    };

    let exemplars = build_json_exemplars(point.exemplars)?;

    let parts = SumRecordParts {
        time_unix_nano: json_timestamp_to_i64(&point.time_unix_nano, "sum.time_unix_nano")?,
        start_time_unix_nano: json_timestamp_to_i64(
            &point.start_time_unix_nano,
            "sum.start_time_unix_nano",
        )?,
        metric_name: ctx.metric_name.clone(),
        metric_description: ctx.metric_description.clone(),
        metric_unit: ctx.metric_unit.clone(),
        value,
        attributes: json_attrs_to_value(point.attributes),
        resource: Arc::clone(&ctx.resource),
        scope: Arc::clone(&ctx.scope),
        flags: point.flags as i64,
        exemplars,
        aggregation_temporality,
        is_monotonic,
    };

    Ok(Some(build_sum_record(parts)))
}

/// Extract numeric value, always producing Float for schema compatibility (Cloudflare expects float64)
/// Returns None for missing values or non-finite numbers (NaN/Infinity)
fn extract_number_value(
    as_int: &Option<JsonNumberOrString>,
    as_double: &Option<f64>,
) -> Option<VrlValue> {
    if let Some(i) = as_int {
        let value = i
            .as_i64()
            .map(|n| finite_float_to_vrl(n as f64, "json.value"))?;
        // finite_float_to_vrl returns Null for NaN/Infinity
        if matches!(value, VrlValue::Null) {
            None
        } else {
            Some(value)
        }
    } else if let Some(d) = as_double {
        let value = finite_float_to_vrl(*d, "json.value");
        // finite_float_to_vrl returns Null for NaN/Infinity
        if matches!(value, VrlValue::Null) {
            None
        } else {
            Some(value)
        }
    } else {
        None
    }
}

fn build_json_exemplars(exemplars: Vec<JsonExemplar>) -> Result<Vec<ExemplarParts>, DecodeError> {
    exemplars
        .into_iter()
        .map(|e| {
            // Exemplar values can be null - they're supplementary metadata
            let value = extract_number_value(&e.as_int, &e.as_double).unwrap_or(VrlValue::Null);
            Ok(ExemplarParts {
                time_unix_nano: json_timestamp_to_i64(
                    &e.time_unix_nano,
                    "exemplar.time_unix_nano",
                )?,
                value,
                trace_id: Bytes::from(decode_bytes_field(&e.trace_id)),
                span_id: Bytes::from(decode_bytes_field(&e.span_id)),
                filtered_attributes: json_attrs_to_value(e.filtered_attributes),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gauge_json(as_double: Option<f64>, as_int: Option<&str>) -> String {
        let value_field = match (as_double, as_int) {
            (Some(d), _) => format!(r#""asDouble": {}"#, d),
            (_, Some(i)) => format!(r#""asInt": "{}""#, i),
            (None, None) => String::new(),
        };

        format!(
            r#"{{
                "resourceMetrics": [{{
                    "resource": {{}},
                    "scopeMetrics": [{{
                        "scope": {{}},
                        "metrics": [{{
                            "name": "test.gauge",
                            "gauge": {{
                                "dataPoints": [{{
                                    "timeUnixNano": "1000000000"
                                    {}
                                }}]
                            }}
                        }}]
                    }}]
                }}]
            }}"#,
            if value_field.is_empty() {
                String::new()
            } else {
                format!(", {}", value_field)
            }
        )
    }

    // Note: NaN/Infinity tests are not applicable for JSON since JSON doesn't
    // support those literals. Those edge cases are tested in metrics_proto.rs.

    #[test]
    fn skips_gauge_with_missing_value() {
        let json = make_gauge_json(None, None);
        let result = decode_json(&Bytes::from(json)).unwrap();
        assert_eq!(result.len(), 0, "Missing value should be skipped");
    }

    #[test]
    fn accepts_gauge_with_valid_double() {
        let json = make_gauge_json(Some(42.5), None);
        let result = decode_json(&Bytes::from(json)).unwrap();
        assert_eq!(result.len(), 1, "Valid double should be accepted");
    }

    #[test]
    fn accepts_gauge_with_valid_int() {
        let json = make_gauge_json(None, Some("42"));
        let result = decode_json(&Bytes::from(json)).unwrap();
        assert_eq!(result.len(), 1, "Valid int should be accepted");
    }
}
