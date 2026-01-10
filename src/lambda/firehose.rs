//! Firehose client implementing PipelineSender trait with retry logic.

use aws_sdk_firehose::{types::Record, Client as AwsClient};
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, warn};
use vrl::value::Value;

use crate::pipeline::{PipelineSender, SendResult};

const MAX_RECORDS_PER_BATCH: usize = 500; // Firehose limit
const MAX_RETRIES: usize = 3;
const BASE_DELAY_MS: u64 = 100;
const MAX_DELAY_MS: u64 = 10_000;

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
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            logs: std::env::var("PIPELINE_LOGS")?,
            traces: std::env::var("PIPELINE_TRACES")?,
            sum: std::env::var("PIPELINE_SUM")?,
            gauge: std::env::var("PIPELINE_GAUGE")?,
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

/// Calculate exponential backoff with jitter.
pub fn calculate_backoff(attempt: usize) -> u64 {
    let base = BASE_DELAY_MS.saturating_mul(2_u64.saturating_pow(attempt as u32));
    let jitter = rand::thread_rng().gen_range(0..=base / 2);
    base.saturating_add(jitter).min(MAX_DELAY_MS)
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
    async fn send_to_stream(
        &self,
        stream_name: &str,
        records: Vec<Value>,
    ) -> Result<usize, String> {
        let mut total_succeeded = 0;
        let mut final_failed: Vec<Value> = Vec::new();

        for chunk in records.chunks(MAX_RECORDS_PER_BATCH) {
            let mut pending: Vec<Value> = chunk.to_vec();

            for attempt in 0..=MAX_RETRIES {
                if pending.is_empty() {
                    break;
                }

                if attempt > 0 {
                    let delay = calculate_backoff(attempt);
                    debug!(attempt, delay_ms = delay, "retrying after backoff");
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }

                // Convert to Firehose records (one JSON object per record, newline-delimited)
                let firehose_records: Vec<Record> = pending
                    .iter()
                    .filter_map(|json_obj| {
                        let mut data = serde_json::to_vec(json_obj).ok()?;
                        data.push(b'\n'); // NDJSON format
                        Record::builder()
                            .data(aws_sdk_firehose::primitives::Blob::new(data))
                            .build()
                            .ok()
                    })
                    .collect();

                let response = self
                    .client
                    .put_record_batch()
                    .delivery_stream_name(stream_name)
                    .set_records(Some(firehose_records))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                let failed_count = response.failed_put_count();
                if failed_count == 0 {
                    total_succeeded += pending.len();
                    pending.clear();
                } else {
                    // Extract failed records for retry
                    let mut new_pending = Vec::new();
                    for (resp, record) in response.request_responses().iter().zip(pending.drain(..))
                    {
                        if resp.error_code().is_some() {
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
