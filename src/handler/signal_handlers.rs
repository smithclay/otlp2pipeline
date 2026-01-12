use bytes::Bytes;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tracing::warn;

use crate::signal::Signal;
use crate::InputFormat;
use otlp2records::{transform_logs_json, transform_metrics_json, transform_traces_json};

use super::{SignalHandler, SkippedMetricsWarning, TransformResult};

/// Handler for OTLP logs
pub struct LogsHandler;

impl SignalHandler for LogsHandler {
    const SIGNAL: Signal = Signal::Logs;

    fn transform(body: Bytes, format: InputFormat) -> Result<TransformResult, otlp2records::Error> {
        let transformed = transform_logs_json(&body, format)?;
        let mut grouped = HashMap::new();
        if !transformed.is_empty() {
            grouped.insert(Signal::Logs.table_name().to_string(), transformed);
        }
        Ok(TransformResult {
            grouped,
            skipped: None,
        })
    }
}

/// Handler for OTLP traces
pub struct TracesHandler;

impl SignalHandler for TracesHandler {
    const SIGNAL: Signal = Signal::Traces;

    fn transform(body: Bytes, format: InputFormat) -> Result<TransformResult, otlp2records::Error> {
        let transformed = transform_traces_json(&body, format)?;
        let mut grouped = HashMap::new();
        if !transformed.is_empty() {
            grouped.insert(Signal::Traces.table_name().to_string(), transformed);
        }
        Ok(TransformResult {
            grouped,
            skipped: None,
        })
    }
}

/// Handler for OTLP metrics (gauge and sum)
pub struct MetricsHandler;

impl MetricsHandler {
    fn insert_if_not_empty(
        grouped: &mut HashMap<String, Vec<JsonValue>>,
        table: &str,
        values: Vec<JsonValue>,
    ) {
        if !values.is_empty() {
            grouped.insert(table.to_string(), values);
        }
    }
}

impl SignalHandler for MetricsHandler {
    const SIGNAL: Signal = Signal::Gauge;

    fn transform(body: Bytes, format: InputFormat) -> Result<TransformResult, otlp2records::Error> {
        let metric_values = transform_metrics_json(&body, format)?;

        // Build warning if any metrics were skipped
        let skipped = if metric_values.skipped.has_skipped() {
            warn!(
                skipped_total = metric_values.skipped.total(),
                histograms = metric_values.skipped.histograms,
                exponential_histograms = metric_values.skipped.exponential_histograms,
                summaries = metric_values.skipped.summaries,
                nan_values = metric_values.skipped.nan_values,
                infinity_values = metric_values.skipped.infinity_values,
                missing_values = metric_values.skipped.missing_values,
                "skipped unsupported or invalid metrics"
            );
            Some(SkippedMetricsWarning {
                message: "some metrics were skipped",
                skipped_total: metric_values.skipped.total(),
                histograms: metric_values.skipped.histograms,
                exponential_histograms: metric_values.skipped.exponential_histograms,
                summaries: metric_values.skipped.summaries,
                nan_values: metric_values.skipped.nan_values,
                infinity_values: metric_values.skipped.infinity_values,
                missing_values: metric_values.skipped.missing_values,
            })
        } else {
            None
        };

        let mut grouped = HashMap::new();
        Self::insert_if_not_empty(
            &mut grouped,
            Signal::Gauge.table_name(),
            metric_values.gauge,
        );
        Self::insert_if_not_empty(&mut grouped, Signal::Sum.table_name(), metric_values.sum);

        Ok(TransformResult { grouped, skipped })
    }
}
