#[cfg(target_arch = "wasm32")]
use crate::livetail::LiveTailSender;
use bytes::Bytes;
use flate2::read::GzDecoder;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::collections::HashSet;
use std::io::Read;
use tracing::{debug, error, info, warn, Span};

use crate::pipeline::PipelineSender;
use crate::signal::Signal;
use crate::InputFormat;

mod signal_handlers;

pub use signal_handlers::{LogsHandler, MetricsHandler, TracesHandler};

const MAX_DECOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug)]
pub enum HandleError {
    Decompress(String),
    Decode(String),
    Transform(String),
    SendFailed(String),
}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleError::Decompress(e) => write!(f, "decompress error: {}", e),
            HandleError::Decode(e) => write!(f, "decode error: {}", e),
            HandleError::Transform(e) => write!(f, "transform error: {}", e),
            HandleError::SendFailed(e) => write!(f, "send failed: {}", e),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct HandleResponse {
    pub status: &'static str,
    pub records: HashMap<String, usize>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub errors: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<SkippedMetricsWarning>,
    #[serde(skip)]
    pub service_names: Vec<String>,
    #[serde(skip)]
    pub metric_names: Vec<(String, String)>,
}

/// Warning info for skipped metrics, surfaced to users in the response
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkippedMetricsWarning {
    pub message: &'static str,
    pub skipped_total: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub histograms: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub exponential_histograms: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub summaries: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub nan_values: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub infinity_values: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub missing_values: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl HandleResponse {
    pub fn empty() -> Self {
        Self {
            status: "ok",
            records: HashMap::new(),
            errors: HashMap::new(),
            warnings: None,
            service_names: Vec::new(),
            metric_names: Vec::new(),
        }
    }

    pub fn from_result(result: crate::pipeline::SendResult) -> Self {
        let status = if result.failed.is_empty() {
            "ok"
        } else if result.succeeded.is_empty() {
            "error"
        } else {
            "partial"
        };

        Self {
            status,
            records: result.succeeded,
            errors: result.failed,
            warnings: None,
            service_names: Vec::new(),
            metric_names: Vec::new(),
        }
    }

    pub fn with_service_names(mut self, service_names: Vec<String>) -> Self {
        self.service_names = service_names;
        self
    }

    pub fn with_metric_names(mut self, metric_names: Vec<(String, String)>) -> Self {
        self.metric_names = metric_names;
        self
    }

    pub fn with_warnings(mut self, warnings: Option<SkippedMetricsWarning>) -> Self {
        self.warnings = warnings;
        self
    }
}

/// Result of transforming a signal payload
pub struct TransformResult {
    pub grouped: HashMap<String, Vec<JsonValue>>,
    pub skipped: Option<SkippedMetricsWarning>,
}

/// Trait for signal-specific decode and transform logic
pub trait SignalHandler {
    /// Which signal type this handler processes
    const SIGNAL: Signal;

    /// Decode and transform a payload into table-grouped JSON records.
    fn transform(body: Bytes, format: InputFormat) -> Result<TransformResult, otlp2records::Error>;
}

/// Extract unique service names from grouped records
#[cfg(target_arch = "wasm32")]
fn extract_service_names(grouped: &HashMap<String, Vec<JsonValue>>) -> Vec<String> {
    let mut service_names = HashSet::new();

    for values in grouped.values() {
        for value in values {
            if let Some(service_name) = value.get("service_name").and_then(|v| v.as_str()) {
                service_names.insert(service_name.to_string());
            }
        }
    }

    service_names.into_iter().collect()
}

/// Extract unique (metric_name, metric_type) pairs from grouped records.
/// Uses _table field as the metric type since _metric_type is cleared by VRL.
#[cfg(target_arch = "wasm32")]
fn extract_metric_names(grouped: &HashMap<String, Vec<JsonValue>>) -> Vec<(String, String)> {
    let mut metric_names = HashSet::new();

    // Only process metric tables (gauge, sum, histogram, exp_histogram, summary)
    let metric_tables = ["gauge", "sum", "histogram", "exp_histogram", "summary"];

    for (table, values) in grouped {
        if !metric_tables.contains(&table.as_str()) {
            continue;
        }

        for value in values {
            if let Some(name) = value.get("metric_name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    metric_names.insert((name.to_string(), table.clone()));
                }
            }
        }
    }

    metric_names.into_iter().collect()
}

pub(crate) fn decompress_if_gzipped(body: Bytes, is_gzipped: bool) -> Result<Bytes, HandleError> {
    if !is_gzipped && body.len() > MAX_DECOMPRESSED_SIZE {
        error!(
            bytes_read = body.len(),
            max = MAX_DECOMPRESSED_SIZE,
            "uncompressed body exceeds limit"
        );
        return Err(HandleError::Decompress(format!(
            "exceeds {}MB limit",
            MAX_DECOMPRESSED_SIZE / 1024 / 1024
        )));
    }

    if is_gzipped {
        debug!(compressed_size = body.len(), "decompressing gzipped body");
        let decoder = GzDecoder::new(body.as_ref());
        let mut decompressed = Vec::with_capacity(body.len().saturating_mul(2));
        let bytes_read = decoder
            .take((MAX_DECOMPRESSED_SIZE + 1) as u64)
            .read_to_end(&mut decompressed)
            .map_err(|e| {
                error!(error = %e, "gzip decompression failed");
                HandleError::Decompress(e.to_string())
            })?;
        if bytes_read > MAX_DECOMPRESSED_SIZE {
            error!(
                bytes_read,
                max = MAX_DECOMPRESSED_SIZE,
                "decompressed size exceeds limit"
            );
            return Err(HandleError::Decompress(format!(
                "exceeds {}MB limit",
                MAX_DECOMPRESSED_SIZE / 1024 / 1024
            )));
        }
        debug!(decompressed_size = bytes_read, "decompression complete");
        Ok(Bytes::from(decompressed))
    } else {
        Ok(body)
    }
}

/// Generic handler for any signal type
#[tracing::instrument(
    name = "ingest",
    skip(body, sender),
    fields(
        signal = ?H::SIGNAL,
        format = ?format,
        gzipped = is_gzipped,
        records = tracing::field::Empty,
        tables = tracing::field::Empty,
    )
)]
pub async fn handle_signal<H: SignalHandler, S: PipelineSender>(
    body: Bytes,
    is_gzipped: bool,
    format: InputFormat,
    sender: &S,
) -> Result<HandleResponse, HandleError> {
    debug!(
        body_size = body.len(),
        is_gzipped,
        signal = ?H::SIGNAL,
        "handling signal request"
    );

    let body = decompress_if_gzipped(body, is_gzipped)?;

    let transform_result = H::transform(body, format).map_err(|e| match e {
        otlp2records::Error::Decode(err) => {
            error!(error = %err, "failed to decode payload");
            HandleError::Decode(err.to_string())
        }
        err => {
            error!(error = %err, "transform failed");
            HandleError::Transform(err.to_string())
        }
    })?;

    let grouped = transform_result.grouped;
    let skipped = transform_result.skipped;

    if grouped.is_empty() {
        debug!("no records to send");
        return Ok(HandleResponse::empty().with_warnings(skipped));
    }

    let table_counts: Vec<_> = grouped.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
    debug!(?table_counts, "sending records to pipelines");

    // Calculate span fields before sending (send_all takes ownership)
    let total_records: usize = grouped.values().map(|v| v.len()).sum();
    let table_names: String = grouped.keys().cloned().collect::<Vec<_>>().join(",");

    let result = sender.send_all(grouped).await;

    if !result.failed.is_empty() {
        for (table, err) in &result.failed {
            warn!(table, error = %err, "pipeline send failed");
        }
    }

    info!(
        succeeded = result.succeeded.len(),
        failed = result.failed.len(),
        signal = ?H::SIGNAL,
        "request complete"
    );

    Span::current().record("records", total_records);
    Span::current().record("tables", &table_names);

    Ok(HandleResponse::from_result(result).with_warnings(skipped))
}

/// Handle signal with optional aggregator dual-write and livetail.
///
/// This function extends handle_signal to support triple-writing telemetry to:
/// 1. Pipeline (cold storage: R2/Iceberg) - required, failures fail the request
/// 2. Aggregator (Durable Objects) - optional best-effort, failures are logged but don't fail the request
/// 3. LiveTail (Durable Objects) - optional best-effort, failures are logged but don't fail the request
#[cfg(target_arch = "wasm32")]
#[tracing::instrument(
    name = "ingest_triple",
    skip(body, sender, cache, livetail),
    fields(
        signal = ?H::SIGNAL,
        format = ?format,
        gzipped = is_gzipped,
        records = tracing::field::Empty,
        tables = tracing::field::Empty,
        cache_enabled = cache.is_some(),
        livetail_enabled = livetail.is_some(),
    )
)]
pub async fn handle_signal_with_cache<H, S, C, L>(
    body: Bytes,
    is_gzipped: bool,
    format: InputFormat,
    sender: &S,
    cache: Option<&C>,
    livetail: Option<&L>,
) -> Result<HandleResponse, HandleError>
where
    H: SignalHandler,
    S: PipelineSender,
    C: crate::aggregator::AggregatorSender,
    L: LiveTailSender,
{
    debug!(
        body_size = body.len(),
        is_gzipped,
        signal = ?H::SIGNAL,
        cache_enabled = cache.is_some(),
        livetail_enabled = livetail.is_some(),
        "handling signal request with triple-write"
    );

    // Decompress
    let body = decompress_if_gzipped(body, is_gzipped)?;

    // Transform
    let transform_result = H::transform(body, format).map_err(|e| match e {
        otlp2records::Error::Decode(err) => {
            error!(error = %err, "failed to decode payload");
            HandleError::Decode(err.to_string())
        }
        err => {
            error!(error = %err, "transform failed");
            HandleError::Transform(err.to_string())
        }
    })?;

    let grouped = transform_result.grouped;
    let skipped = transform_result.skipped;

    if grouped.is_empty() {
        debug!("no records to send");
        return Ok(HandleResponse::empty().with_warnings(skipped));
    }

    let table_counts: Vec<_> = grouped.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
    debug!(?table_counts, "sending records to pipelines");

    // Calculate span fields before sending (send_all takes ownership)
    let total_records: usize = grouped.values().map(|v| v.len()).sum();
    let table_names: String = grouped.keys().cloned().collect::<Vec<_>>().join(",");

    // Extract service names and metric names before sending (send_all takes ownership)
    let service_names = extract_service_names(&grouped);
    let metric_names = extract_metric_names(&grouped);

    // Triple-write: pipeline is primary (required), aggregator and livetail are best-effort (optional)
    let pipeline_result = match (cache, livetail) {
        (Some(cache), Some(livetail)) => {
            // Clone grouped data for aggregator and livetail writes
            let grouped_agg = grouped.clone();
            let grouped_tail = grouped.clone();

            // Send to all three in parallel
            let (p_result, a_result, l_result) = futures::join!(
                sender.send_all(grouped),
                cache.send_to_aggregator(grouped_agg),
                livetail.send_to_livetail(grouped_tail)
            );

            // Log aggregator errors but don't fail the request
            if !a_result.failed.is_empty() {
                for (do_name, error) in &a_result.failed {
                    warn!(do_name = %do_name, error = %error, "aggregator write failed");
                }
            } else {
                debug!(
                    succeeded = a_result.succeeded.len(),
                    "aggregator write succeeded"
                );
            }

            // Log livetail errors but don't fail the request
            if !l_result.errors.is_empty() {
                for (do_name, error) in &l_result.errors {
                    warn!(do_name = %do_name, error = %error, "livetail write failed");
                }
            } else {
                debug!(sent = l_result.sent.len(), "livetail write succeeded");
            }

            p_result
        }
        (Some(cache), None) => {
            // Clone grouped data for aggregator write
            let grouped_clone = grouped.clone();

            // Send to pipeline and aggregator in parallel
            let (p_result, a_result) = futures::join!(
                sender.send_all(grouped),
                cache.send_to_aggregator(grouped_clone)
            );

            // Log aggregator errors but don't fail the request
            if !a_result.failed.is_empty() {
                for (do_name, error) in &a_result.failed {
                    warn!(do_name = %do_name, error = %error, "aggregator write failed");
                }
            } else {
                debug!(
                    succeeded = a_result.succeeded.len(),
                    "aggregator write succeeded"
                );
            }

            p_result
        }
        (None, Some(livetail)) => {
            // Clone grouped data for livetail write
            let grouped_clone = grouped.clone();

            // Send to pipeline and livetail in parallel
            let (p_result, l_result) = futures::join!(
                sender.send_all(grouped),
                livetail.send_to_livetail(grouped_clone)
            );

            // Log livetail errors but don't fail the request
            if !l_result.errors.is_empty() {
                for (do_name, error) in &l_result.errors {
                    warn!(do_name = %do_name, error = %error, "livetail write failed");
                }
            } else {
                debug!(sent = l_result.sent.len(), "livetail write succeeded");
            }

            p_result
        }
        (None, None) => {
            // Just send to pipeline
            sender.send_all(grouped).await
        }
    };

    // Pipeline failure = request failure
    if !pipeline_result.failed.is_empty() {
        for (table, err) in &pipeline_result.failed {
            warn!(table = %table, error = %err, "pipeline send failed");
        }

        let errors: Vec<String> = pipeline_result
            .failed
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect();
        return Err(HandleError::SendFailed(errors.join("; ")));
    }

    info!(
        succeeded = pipeline_result.succeeded.len(),
        signal = ?H::SIGNAL,
        "request complete"
    );

    Span::current().record("records", total_records);
    Span::current().record("tables", &table_names);

    Ok(HandleResponse::from_result(pipeline_result)
        .with_service_names(service_names)
        .with_metric_names(metric_names)
        .with_warnings(skipped))
}
