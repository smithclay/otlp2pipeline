use bytes::Bytes;
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::io::Read;
use tracing::{debug, error, info, warn, Span};
use vrl::value::Value;

use crate::decode::DecodeFormat;
use crate::pipeline::PipelineSender;
use crate::signal::Signal;
use crate::transform::{VrlError, VrlTransformer};

mod signal_handlers;

pub use signal_handlers::{HecLogsHandler, LogsHandler, MetricsHandler, TracesHandler};

const MAX_DECOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct DecodeError(pub String);

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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
}

impl HandleResponse {
    pub fn empty() -> Self {
        Self {
            status: "ok",
            records: HashMap::new(),
            errors: HashMap::new(),
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
        }
    }
}

/// Trait for signal-specific decode and transform logic
pub trait SignalHandler {
    /// Which signal type this handler processes
    const SIGNAL: Signal;

    /// Decode raw bytes into VRL values
    fn decode(body: Bytes, format: DecodeFormat) -> Result<Vec<Value>, DecodeError>;

    /// Get the VRL program for transformation
    fn vrl_program() -> &'static vrl::compiler::Program;

    /// Transform a batch of values. Default uses vrl_program().
    /// Override for handlers needing multiple programs (e.g., metrics partitioning).
    fn transform_batch(
        transformer: &mut VrlTransformer,
        values: Vec<Value>,
    ) -> Result<HashMap<String, Vec<Value>>, VrlError> {
        transformer.transform_batch(Self::vrl_program(), values)
    }
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
    format: DecodeFormat,
    sender: &S,
) -> Result<HandleResponse, HandleError> {
    debug!(
        body_size = body.len(),
        is_gzipped,
        signal = ?H::SIGNAL,
        "handling signal request"
    );

    let body = decompress_if_gzipped(body, is_gzipped)?;

    let values = H::decode(body, format).map_err(|e| {
        error!(error = %e, "failed to decode payload");
        HandleError::Decode(e.0)
    })?;

    if values.is_empty() {
        debug!("no records to transform");
        return Ok(HandleResponse::empty());
    }

    debug!(record_count = values.len(), "transforming records");
    let mut transformer = VrlTransformer::new();
    let grouped = H::transform_batch(&mut transformer, values).map_err(|e| {
        error!(error = %e, "VRL transform failed");
        HandleError::Transform(e.to_string())
    })?;

    if grouped.is_empty() {
        debug!("no records to send");
        return Ok(HandleResponse::empty());
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

    Ok(HandleResponse::from_result(result))
}

/// Handle signal with optional hot cache dual-write.
///
/// This function extends handle_signal to support dual-writing telemetry to both:
/// 1. Pipeline (cold storage: R2/Iceberg) - required, failures fail the request
/// 2. Hot Cache (Durable Objects) - optional best-effort, failures are logged but don't fail the request
#[cfg(target_arch = "wasm32")]
#[tracing::instrument(
    name = "ingest_dual",
    skip(body, sender, cache),
    fields(
        signal = ?H::SIGNAL,
        format = ?format,
        gzipped = is_gzipped,
        records = tracing::field::Empty,
        tables = tracing::field::Empty,
        cache_enabled = cache.is_some(),
    )
)]
pub async fn handle_signal_with_cache<H, S, C>(
    body: Bytes,
    is_gzipped: bool,
    format: DecodeFormat,
    sender: &S,
    cache: Option<&C>,
) -> Result<HandleResponse, HandleError>
where
    H: SignalHandler,
    S: PipelineSender,
    C: crate::cache::HotCacheSender,
{
    debug!(
        body_size = body.len(),
        is_gzipped,
        signal = ?H::SIGNAL,
        cache_enabled = cache.is_some(),
        "handling signal request with dual-write"
    );

    // Decompress
    let body = decompress_if_gzipped(body, is_gzipped)?;

    // Decode
    let values = H::decode(body, format).map_err(|e| {
        error!(error = %e, "failed to decode payload");
        HandleError::Decode(e.0)
    })?;

    if values.is_empty() {
        debug!("no records to transform");
        return Ok(HandleResponse::empty());
    }

    // Transform
    debug!(record_count = values.len(), "transforming records");
    let mut transformer = VrlTransformer::new();
    let grouped = H::transform_batch(&mut transformer, values).map_err(|e| {
        error!(error = %e, "VRL transform failed");
        HandleError::Transform(e.to_string())
    })?;

    if grouped.is_empty() {
        debug!("no records to send");
        return Ok(HandleResponse::empty());
    }

    let table_counts: Vec<_> = grouped.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
    debug!(?table_counts, "sending records to pipelines");

    // Calculate span fields before sending (send_all takes ownership)
    let total_records: usize = grouped.values().map(|v| v.len()).sum();
    let table_names: String = grouped.keys().cloned().collect::<Vec<_>>().join(",");

    // Dual-write: pipeline is primary (required), cache is best-effort (optional)
    let pipeline_result = if let Some(cache) = cache {
        // Clone grouped data for cache write
        let grouped_clone = grouped.clone();

        // Send to pipeline first (takes ownership)
        let p_result = sender.send_all(grouped).await;

        // Send to cache (best-effort, don't block on failure)
        let c_result = cache.send_all(grouped_clone).await;

        // Log cache errors but don't fail the request
        if !c_result.failed.is_empty() {
            for (table, error) in &c_result.failed {
                warn!(table = %table, error = %error, "hot cache write failed");
            }
        } else {
            debug!(
                succeeded = c_result.succeeded.len(),
                "hot cache write succeeded"
            );
        }

        p_result
    } else {
        sender.send_all(grouped).await
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

    Ok(HandleResponse::from_result(pipeline_result))
}
