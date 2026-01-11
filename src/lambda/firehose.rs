//! Firehose client implementing PipelineSender trait with retry logic.

use aws_sdk_firehose::{
    error::ProvideErrorMetadata, operation::RequestId, types::Record, Client as AwsClient,
};
use std::collections::HashMap;
use tracing::{debug, error, warn};
use vrl::value::Value;

use crate::pipeline::retry::RetryConfig;
use crate::pipeline::{PipelineSender, SendResult};

const MAX_RECORDS_PER_BATCH: usize = 500; // Firehose limit

/// Default retry configuration for Firehose operations.
/// Uses exponential backoff with jitter (100ms base, 10s max, 3 attempts).
fn default_retry_config() -> RetryConfig {
    RetryConfig::exponential(3, 100, 10_000)
}

/// Firehose delivery stream configuration per signal type.
#[derive(Clone)]
pub struct StreamConfig {
    pub logs: String,
    pub traces: String,
    pub sum: String,
    pub gauge: String,
}

impl StreamConfig {
    /// Load stream names from environment variables.
    /// Returns an error message including the missing variable name.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            logs: std::env::var("PIPELINE_LOGS")
                .map_err(|_| "PIPELINE_LOGS environment variable not set")?,
            traces: std::env::var("PIPELINE_TRACES")
                .map_err(|_| "PIPELINE_TRACES environment variable not set")?,
            sum: std::env::var("PIPELINE_SUM")
                .map_err(|_| "PIPELINE_SUM environment variable not set")?,
            gauge: std::env::var("PIPELINE_GAUGE")
                .map_err(|_| "PIPELINE_GAUGE environment variable not set")?,
        })
    }

    /// Get stream name for a table.
    pub fn stream_for_table(&self, table: &str) -> Option<&str> {
        match table {
            "logs" => Some(&self.logs),
            "traces" => Some(&self.traces),
            "sum" => Some(&self.sum),
            "gauge" => Some(&self.gauge),
            _ => None,
        }
    }
}

/// Firehose client that implements PipelineSender.
pub struct FirehoseSender {
    client: AwsClient,
    streams: StreamConfig,
}

impl FirehoseSender {
    /// Create a new FirehoseSender from AWS config.
    pub async fn new(streams: StreamConfig) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Self {
            client: AwsClient::new(&config),
            streams,
        }
    }

    /// Send records to a single Firehose stream with retry.
    /// Retries both API-level errors (throttling, network) and partial failures.
    async fn send_to_stream(
        &self,
        stream_name: &str,
        records: Vec<Value>,
    ) -> Result<usize, String> {
        let retry_config = default_retry_config();
        let max_attempts = retry_config.max_attempts;
        let mut total_succeeded = 0;
        let mut final_failed: Vec<Value> = Vec::new();

        for chunk in records.chunks(MAX_RECORDS_PER_BATCH) {
            let mut pending: Vec<Value> = chunk.to_vec();

            for attempt in 0..max_attempts {
                if pending.is_empty() {
                    break;
                }

                if attempt > 0 {
                    let delay = retry_config.delay_for_attempt(attempt - 1);
                    debug!(
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        "retrying after backoff"
                    );
                    tokio::time::sleep(delay).await;
                }

                // Convert to Firehose records (one JSON object per record, newline-delimited)
                // Fail fast if any record cannot be serialized to maintain 1:1 correspondence
                let firehose_records: Vec<Record> = pending
                    .iter()
                    .map(|json_obj| {
                        let mut data = serde_json::to_vec(json_obj)
                            .map_err(|e| format!("JSON serialization failed: {e}"))?;
                        data.push(b'\n'); // NDJSON format
                        Record::builder()
                            .data(aws_sdk_firehose::primitives::Blob::new(data))
                            .build()
                            .map_err(|e| format!("Record build failed: {e}"))
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                let response = match self
                    .client
                    .put_record_batch()
                    .delivery_stream_name(stream_name)
                    .set_records(Some(firehose_records))
                    .send()
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => {
                        // Log detailed error info including request ID for AWS support
                        let request_id = e.meta().request_id().unwrap_or("unknown");
                        error!(
                            error = %e,
                            request_id = request_id,
                            stream = stream_name,
                            attempt = attempt,
                            "Firehose API call failed"
                        );
                        // Retry API-level errors (throttling, network issues)
                        if attempt + 1 < max_attempts {
                            warn!(
                                attempt,
                                stream = stream_name,
                                "Firehose API error, will retry"
                            );
                            continue;
                        }
                        return Err(format!(
                            "Firehose API error after {} attempts: {}",
                            max_attempts, e
                        ));
                    }
                };

                let failed_count = response.failed_put_count();
                if failed_count == 0 {
                    total_succeeded += pending.len();
                    pending.clear();
                } else {
                    // Extract failed records for retry, logging first error for debugging
                    let mut new_pending = Vec::new();
                    let mut first_error_logged = false;
                    for (resp, record) in response.request_responses().iter().zip(pending.drain(..))
                    {
                        if resp.error_code().is_some() {
                            // Log first error code/message for debugging
                            if !first_error_logged {
                                warn!(
                                    error_code = resp.error_code().unwrap_or("unknown"),
                                    error_message = resp.error_message().unwrap_or("none"),
                                    stream = stream_name,
                                    "Firehose record failure"
                                );
                                first_error_logged = true;
                            }
                            new_pending.push(record);
                        } else {
                            total_succeeded += 1;
                        }
                    }
                    pending = new_pending;

                    warn!(
                        attempt,
                        failed = pending.len(),
                        stream = stream_name,
                        "Firehose partial failure, retrying"
                    );
                }
            }

            // Any remaining pending records go to final_failed
            final_failed.extend(pending);
        }

        if !final_failed.is_empty() {
            error!(
                failed_count = final_failed.len(),
                stream = stream_name,
                "Records failed after retry exhaustion"
            );
            return Err(format!(
                "{} records failed after retries",
                final_failed.len()
            ));
        }

        Ok(total_succeeded)
    }
}

#[async_trait::async_trait]
impl PipelineSender for FirehoseSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult {
        let mut result = SendResult::default();

        for (table, records) in grouped {
            let stream_name = match self.streams.stream_for_table(&table) {
                Some(name) => name,
                None => {
                    warn!(table = %table, "no stream configured for table");
                    result
                        .failed
                        .insert(table, "no stream configured".to_string());
                    continue;
                }
            };

            match self.send_to_stream(stream_name, records).await {
                Ok(count) => {
                    result.succeeded.insert(table, count);
                }
                Err(e) => {
                    result.failed.insert(table, e);
                }
            }
        }

        result
    }
}
