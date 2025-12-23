use bytes::Bytes;
use const_hex::encode as hex_encode;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::metrics::v1::metric::Data;
use opentelemetry_proto::tonic::metrics::v1::number_data_point::Value;
use prost::Message;
use std::sync::Arc;
use tracing::debug;
use vrl::value::Value as VrlValue;

use super::common::{
    finite_float_to_vrl, for_each_resource_scope, otlp_attributes_to_value, otlp_resource_to_value,
    otlp_scope_to_value, safe_timestamp_conversion, DecodeError,
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

pub fn decode_protobuf(body: &Bytes) -> Result<Vec<VrlValue>, DecodeError> {
    let request =
        ExportMetricsServiceRequest::decode(body.as_ref()).map_err(DecodeError::Protobuf)?;
    export_metrics_to_vrl_proto(request)
}

fn export_metrics_to_vrl_proto(
    request: ExportMetricsServiceRequest,
) -> Result<Vec<VrlValue>, DecodeError> {
    let mut values = preallocate_metric_values(&request.resource_metrics, |rm| {
        rm.scope_metrics
            .iter()
            .flat_map(|sm| sm.metrics.iter())
            .map(count_data_points)
            .sum()
    });

    for_each_resource_scope(
        request.resource_metrics,
        |resource_metrics| {
            (
                otlp_resource_to_value(resource_metrics.resource.as_ref()),
                resource_metrics.scope_metrics,
            )
        },
        |scope_metrics| {
            (
                otlp_scope_to_value(scope_metrics.scope.as_ref()),
                scope_metrics.metrics,
            )
        },
        |metrics, resource, scope| {
            for metric in metrics {
                let ctx = MetricContext {
                    metric_name: Bytes::from(metric.name.clone()),
                    metric_description: Bytes::from(metric.description.clone()),
                    metric_unit: Bytes::from(metric.unit.clone()),
                    resource: Arc::clone(&resource),
                    scope: Arc::clone(&scope),
                };

                match metric.data {
                    Some(Data::Gauge(gauge)) => {
                        for point in gauge.data_points {
                            if let Some(record) = build_gauge_from_point(&point, &ctx)? {
                                values.push(record);
                            }
                        }
                    }
                    Some(Data::Sum(sum)) => {
                        let aggregation_temporality = sum.aggregation_temporality as i64;
                        let is_monotonic = sum.is_monotonic;
                        for point in sum.data_points {
                            if let Some(record) = build_sum_from_point(
                                &point,
                                &ctx,
                                aggregation_temporality,
                                is_monotonic,
                            )? {
                                values.push(record);
                            }
                        }
                    }
                    Some(Data::Histogram(_)) => {
                        debug!(
                            metric_name = %metric.name,
                            "skipping unsupported metric type: histogram"
                        );
                    }
                    Some(Data::ExponentialHistogram(_)) => {
                        debug!(
                            metric_name = %metric.name,
                            "skipping unsupported metric type: exponential_histogram"
                        );
                    }
                    Some(Data::Summary(_)) => {
                        debug!(
                            metric_name = %metric.name,
                            "skipping unsupported metric type: summary"
                        );
                    }
                    None => {
                        debug!(
                            metric_name = %metric.name,
                            "skipping metric with no data"
                        );
                    }
                }
            }

            Ok(())
        },
    )?;

    Ok(values)
}

fn count_data_points(metric: &opentelemetry_proto::tonic::metrics::v1::Metric) -> usize {
    match &metric.data {
        Some(Data::Gauge(g)) => g.data_points.len(),
        Some(Data::Sum(s)) => s.data_points.len(),
        _ => 0,
    }
}

fn build_gauge_from_point(
    point: &opentelemetry_proto::tonic::metrics::v1::NumberDataPoint,
    ctx: &MetricContext,
) -> Result<Option<VrlValue>, DecodeError> {
    let time_unix_nano = safe_timestamp_conversion(point.time_unix_nano, "gauge.time_unix_nano")?;
    let start_time_unix_nano =
        safe_timestamp_conversion(point.start_time_unix_nano, "gauge.start_time_unix_nano")?;

    // Always produce Float for schema compatibility (Cloudflare expects float64)
    // Skip records with missing or non-finite values (NaN/Infinity)
    let value = match &point.value {
        Some(Value::AsInt(i)) => finite_float_to_vrl(*i as f64, "gauge.value"),
        Some(Value::AsDouble(d)) => finite_float_to_vrl(*d, "gauge.value"),
        None => {
            debug!(metric_name = %String::from_utf8_lossy(&ctx.metric_name), "skipping gauge point with no value");
            return Ok(None);
        }
    };

    // Skip if value is null (NaN/Infinity was converted to null)
    if matches!(value, VrlValue::Null) {
        debug!(metric_name = %String::from_utf8_lossy(&ctx.metric_name), "skipping gauge point with non-finite value");
        return Ok(None);
    }

    let exemplars = build_exemplars(&point.exemplars)?;

    let parts = GaugeRecordParts {
        time_unix_nano,
        start_time_unix_nano,
        metric_name: ctx.metric_name.clone(),
        metric_description: ctx.metric_description.clone(),
        metric_unit: ctx.metric_unit.clone(),
        value,
        attributes: otlp_attributes_to_value(&point.attributes),
        resource: Arc::clone(&ctx.resource),
        scope: Arc::clone(&ctx.scope),
        flags: point.flags as i64,
        exemplars,
    };

    Ok(Some(build_gauge_record(parts)))
}

fn build_sum_from_point(
    point: &opentelemetry_proto::tonic::metrics::v1::NumberDataPoint,
    ctx: &MetricContext,
    aggregation_temporality: i64,
    is_monotonic: bool,
) -> Result<Option<VrlValue>, DecodeError> {
    let time_unix_nano = safe_timestamp_conversion(point.time_unix_nano, "sum.time_unix_nano")?;
    let start_time_unix_nano =
        safe_timestamp_conversion(point.start_time_unix_nano, "sum.start_time_unix_nano")?;

    // Always produce Float for schema compatibility (Cloudflare expects float64)
    // Skip records with missing or non-finite values (NaN/Infinity)
    let value = match &point.value {
        Some(Value::AsInt(i)) => finite_float_to_vrl(*i as f64, "sum.value"),
        Some(Value::AsDouble(d)) => finite_float_to_vrl(*d, "sum.value"),
        None => {
            debug!(metric_name = %String::from_utf8_lossy(&ctx.metric_name), "skipping sum point with no value");
            return Ok(None);
        }
    };

    // Skip if value is null (NaN/Infinity was converted to null)
    if matches!(value, VrlValue::Null) {
        debug!(metric_name = %String::from_utf8_lossy(&ctx.metric_name), "skipping sum point with non-finite value");
        return Ok(None);
    }

    let exemplars = build_exemplars(&point.exemplars)?;

    let parts = SumRecordParts {
        time_unix_nano,
        start_time_unix_nano,
        metric_name: ctx.metric_name.clone(),
        metric_description: ctx.metric_description.clone(),
        metric_unit: ctx.metric_unit.clone(),
        value,
        attributes: otlp_attributes_to_value(&point.attributes),
        resource: Arc::clone(&ctx.resource),
        scope: Arc::clone(&ctx.scope),
        flags: point.flags as i64,
        exemplars,
        aggregation_temporality,
        is_monotonic,
    };

    Ok(Some(build_sum_record(parts)))
}

fn build_exemplars(
    exemplars: &[opentelemetry_proto::tonic::metrics::v1::Exemplar],
) -> Result<Vec<ExemplarParts>, DecodeError> {
    exemplars
        .iter()
        .map(|e| {
            let time_unix_nano =
                safe_timestamp_conversion(e.time_unix_nano, "exemplar.time_unix_nano")?;

            // Always produce Float for schema compatibility
            let value = match &e.value {
                Some(opentelemetry_proto::tonic::metrics::v1::exemplar::Value::AsInt(i)) => {
                    finite_float_to_vrl(*i as f64, "exemplar.value")
                }
                Some(opentelemetry_proto::tonic::metrics::v1::exemplar::Value::AsDouble(d)) => {
                    finite_float_to_vrl(*d, "exemplar.value")
                }
                None => VrlValue::Null,
            };

            Ok(ExemplarParts {
                time_unix_nano,
                value,
                trace_id: Bytes::from(hex_encode(&e.trace_id)),
                span_id: Bytes::from(hex_encode(&e.span_id)),
                filtered_attributes: otlp_attributes_to_value(&e.filtered_attributes),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_proto::tonic::{
        collector::metrics::v1::ExportMetricsServiceRequest,
        common::v1::InstrumentationScope,
        metrics::v1::{Gauge, Metric, NumberDataPoint, ResourceMetrics, ScopeMetrics},
        resource::v1::Resource,
    };

    #[test]
    fn decodes_gauge_metric() {
        let point = NumberDataPoint {
            time_unix_nano: 1_000_000_000,
            start_time_unix_nano: 900_000_000,
            value: Some(Value::AsDouble(42.5)),
            ..Default::default()
        };

        let request = ExportMetricsServiceRequest {
            resource_metrics: vec![ResourceMetrics {
                resource: Some(Resource::default()),
                scope_metrics: vec![ScopeMetrics {
                    scope: Some(InstrumentationScope::default()),
                    metrics: vec![Metric {
                        name: "test.gauge".to_string(),
                        description: "A test gauge".to_string(),
                        unit: "1".to_string(),
                        data: Some(Data::Gauge(Gauge {
                            data_points: vec![point],
                        })),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body));

        assert!(result.is_ok());
        let values = result.unwrap();
        assert_eq!(values.len(), 1);

        if let VrlValue::Object(map) = &values[0] {
            assert_eq!(
                map.get("metric_name"),
                Some(&VrlValue::Bytes(Bytes::from("test.gauge")))
            );
            assert_eq!(
                map.get("_metric_type"),
                Some(&VrlValue::Bytes(Bytes::from("gauge")))
            );
        } else {
            panic!("expected object");
        }
    }

    fn make_gauge_request(points: Vec<NumberDataPoint>) -> ExportMetricsServiceRequest {
        ExportMetricsServiceRequest {
            resource_metrics: vec![ResourceMetrics {
                resource: Some(Resource::default()),
                scope_metrics: vec![ScopeMetrics {
                    scope: Some(InstrumentationScope::default()),
                    metrics: vec![Metric {
                        name: "test.gauge".to_string(),
                        data: Some(Data::Gauge(Gauge {
                            data_points: points,
                        })),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }

    #[test]
    fn skips_gauge_with_nan_value() {
        let point = NumberDataPoint {
            time_unix_nano: 1_000_000_000,
            value: Some(Value::AsDouble(f64::NAN)),
            ..Default::default()
        };

        let request = make_gauge_request(vec![point]);
        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body)).unwrap();

        assert_eq!(result.len(), 0, "NaN value should be skipped");
    }

    #[test]
    fn skips_gauge_with_infinity_value() {
        let point = NumberDataPoint {
            time_unix_nano: 1_000_000_000,
            value: Some(Value::AsDouble(f64::INFINITY)),
            ..Default::default()
        };

        let request = make_gauge_request(vec![point]);
        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body)).unwrap();

        assert_eq!(result.len(), 0, "Infinity value should be skipped");
    }

    #[test]
    fn skips_gauge_with_neg_infinity_value() {
        let point = NumberDataPoint {
            time_unix_nano: 1_000_000_000,
            value: Some(Value::AsDouble(f64::NEG_INFINITY)),
            ..Default::default()
        };

        let request = make_gauge_request(vec![point]);
        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body)).unwrap();

        assert_eq!(result.len(), 0, "Negative infinity value should be skipped");
    }

    #[test]
    fn skips_gauge_with_missing_value() {
        let point = NumberDataPoint {
            time_unix_nano: 1_000_000_000,
            value: None,
            ..Default::default()
        };

        let request = make_gauge_request(vec![point]);
        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body)).unwrap();

        assert_eq!(result.len(), 0, "Missing value should be skipped");
    }

    #[test]
    fn keeps_valid_gauge_skips_invalid() {
        let valid_point = NumberDataPoint {
            time_unix_nano: 1_000_000_000,
            value: Some(Value::AsDouble(42.5)),
            ..Default::default()
        };
        let nan_point = NumberDataPoint {
            time_unix_nano: 2_000_000_000,
            value: Some(Value::AsDouble(f64::NAN)),
            ..Default::default()
        };
        let missing_point = NumberDataPoint {
            time_unix_nano: 3_000_000_000,
            value: None,
            ..Default::default()
        };

        let request = make_gauge_request(vec![valid_point, nan_point, missing_point]);
        let body = request.encode_to_vec();
        let result = decode_protobuf(&Bytes::from(body)).unwrap();

        assert_eq!(result.len(), 1, "Only valid point should be kept");
    }
}
