use crate::pipeline::retry::{with_retry, IsRetryable, RetryConfig};
use crate::pipeline::sender::{PipelineSender, SendResult};
use crate::schema::get_schema;
use crate::signal::Signal;
use bytes::{BufMut, Bytes, BytesMut};
use futures::future::join_all;
use reqwest::Client;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use tracing::info;
use tracing::{debug, error, warn};

#[cfg(not(target_arch = "wasm32"))]
const SEND_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum body size for pipeline requests (Cloudflare limit is 1MB, use 900KB for safety margin)
const MAX_BODY_SIZE: usize = 900 * 1024;

/// Errors that can occur when sending to a pipeline
#[derive(Debug)]
pub enum SendError {
    Timeout,
    Http { status: u16, endpoint: String },
    Network(String),
    Serialize(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Timeout => write!(f, "request timed out"),
            SendError::Http { status, endpoint } => {
                write!(f, "HTTP {} from {}", status, endpoint)
            }
            SendError::Network(msg) => write!(f, "network error: {}", msg),
            SendError::Serialize(msg) => write!(f, "serialization error: {}", msg),
        }
    }
}

impl IsRetryable for SendError {
    fn is_retryable(&self) -> bool {
        match self {
            SendError::Timeout => true,
            SendError::Http { status, .. } => matches!(status, 502..=504),
            SendError::Network(_) => true,
            SendError::Serialize(_) => false,
        }
    }
}

/// Unified pipeline client for both WASM and native targets
pub struct PipelineClient {
    client: Client,
    endpoints: HashMap<Signal, String>,
    token: String,
}

impl PipelineClient {
    /// Create a new client with the given endpoints and auth token.
    /// Returns an error if the HTTP client fails to build (e.g., TLS configuration issues).
    pub fn new(endpoints: HashMap<Signal, String>, token: String) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        let client = Client::builder()
            .timeout(SEND_TIMEOUT)
            .build()
            .map_err(|e| format!("failed to build HTTP client: {}", e))?;
        #[cfg(target_arch = "wasm32")]
        let client = Client::builder()
            .build()
            .map_err(|e| format!("failed to build HTTP client: {}", e))?;
        Ok(Self {
            client,
            endpoints,
            token,
        })
    }

    /// Build from Cloudflare Worker environment
    #[cfg(target_arch = "wasm32")]
    pub fn from_worker_env(env: &worker::Env) -> worker::Result<Self> {
        let token = env.secret("PIPELINE_AUTH_TOKEN")?.to_string();
        let mut endpoints = HashMap::new();

        for signal in Signal::all() {
            if let Ok(v) = env.var(signal.env_var_name()) {
                let url = v.to_string();
                if !url.is_empty() {
                    endpoints.insert(*signal, url);
                }
            }
        }

        info!(
            endpoint_count = endpoints.len(),
            "PipelineClient initialized"
        );
        Self::new(endpoints, token).map_err(|e| worker::Error::RustError(e))
    }

    /// Send records to a pipeline endpoint, automatically chunking if needed to stay under size limit
    #[tracing::instrument(
        name = "pipeline_send",
        skip(self, records),
        fields(
            table = %table,
            record_count = records.len(),
        )
    )]
    async fn send_batch(
        &self,
        table: &str,
        endpoint: &str,
        records: Vec<JsonValue>,
    ) -> Result<usize, SendError> {
        let total_records = records.len();
        debug!(endpoint, total_records, "sending batch to pipeline");

        // Build size-limited batches with schema validation for metrics
        let batches = build_ndjson_batches(&records, MAX_BODY_SIZE, table)?;
        let batch_count = batches.len();

        if batch_count > 1 {
            debug!(
                batch_count,
                total_records, "splitting into multiple batches due to size limit"
            );
        }

        let mut sent_count = 0;
        for (batch_idx, body) in batches.into_iter().enumerate() {
            let batch_size = body.len();
            debug!(batch_idx, batch_size, batch_count, "sending batch chunk");

            sent_count += self.send_single_batch(endpoint, body).await?;
        }

        debug!(endpoint, sent_count, "all batches sent successfully");
        Ok(sent_count)
    }

    /// Send a single pre-built NDJSON body to the pipeline
    async fn send_single_batch(&self, endpoint: &str, body: Bytes) -> Result<usize, SendError> {
        let retry_config = RetryConfig::default();
        // Count records by counting newlines + 1 (NDJSON format)
        let record_count = body.iter().filter(|&&b| b == b'\n').count() + 1;

        with_retry(&retry_config, || async {
            let response = self
                .client
                .post(endpoint)
                .header("Content-Type", "application/x-ndjson")
                .header("Authorization", format!("Bearer {}", self.token))
                .body(body.clone())
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        SendError::Timeout
                    } else {
                        SendError::Network(e.to_string())
                    }
                })?;

            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                // Try to get response body for better error diagnostics
                let resp_body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "(failed to read body)".to_string());
                error!(
                    endpoint,
                    status,
                    response_body = %resp_body,
                    "pipeline returned error status"
                );
                return Err(SendError::Http {
                    status,
                    endpoint: endpoint.to_string(),
                });
            }

            Ok(record_count)
        })
        .await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl PipelineSender for PipelineClient {
    async fn send_all(&self, grouped: HashMap<String, Vec<JsonValue>>) -> SendResult {
        let mut send_result = SendResult::default();
        let mut futures = Vec::new();

        for (table_name, records) in grouped.into_iter().filter(|(_, r)| !r.is_empty()) {
            let signal = match Signal::from_table_name(&table_name) {
                Some(s) => s,
                None => {
                    warn!(table = %table_name, "unknown signal type");
                    send_result
                        .failed
                        .insert(table_name, "unknown signal type".to_string());
                    continue;
                }
            };

            if let Some(endpoint) = self.endpoints.get(&signal) {
                let endpoint = endpoint.clone();
                let table = table_name.clone();
                futures.push(async move {
                    let result = self.send_batch(&table, &endpoint, records).await;
                    (table, result)
                });
            } else {
                warn!(table = %table_name, "no pipeline endpoint configured");
                let message = format!("no pipeline endpoint configured for {}", table_name);
                send_result.failed.insert(table_name, message);
            }
        }

        let results = join_all(futures).await;

        for (table, result) in results {
            match result {
                Ok(count) => {
                    send_result.succeeded.insert(table, count);
                }
                Err(e) => {
                    send_result.failed.insert(table, e.to_string());
                }
            }
        }

        send_result
    }
}

/// Validate a record against its schema before sending.
/// Uses centralized schema definitions from crate::schema.
fn validate_record_schema(json: &JsonValue, table: &str, idx: usize) -> Result<(), SendError> {
    if let Some(schema) = get_schema(table) {
        schema.validate(json, idx).map_err(SendError::Serialize)?;
    }
    Ok(())
}

/// Build NDJSON batches, splitting into multiple batches if total size exceeds max_size
fn build_ndjson_batches(
    records: &[JsonValue],
    max_size: usize,
    table: &str,
) -> Result<Vec<Bytes>, SendError> {
    let mut batches = Vec::new();
    let mut current_buf = BytesMut::new();
    let mut first_in_batch = true;

    for (idx, record) in records.iter().enumerate() {
        // Validate record against schema before serialization
        validate_record_schema(record, table, idx)?;

        let json_bytes =
            serde_json::to_vec(record).map_err(|e| SendError::Serialize(e.to_string()))?;

        // Calculate size this record would add (including newline separator)
        let record_size = if first_in_batch {
            json_bytes.len()
        } else {
            json_bytes.len() + 1 // +1 for newline
        };

        // If adding this record would exceed max size, start a new batch
        // (but always include at least one record per batch)
        if !first_in_batch && current_buf.len() + record_size > max_size {
            batches.push(current_buf.freeze());
            current_buf = BytesMut::new();
            first_in_batch = true;
        }

        if !first_in_batch {
            current_buf.put_slice(b"\n");
        } else {
            first_in_batch = false;
        }
        current_buf.extend_from_slice(&json_bytes);
    }

    // Don't forget the last batch
    if !current_buf.is_empty() {
        batches.push(current_buf.freeze());
    }

    Ok(batches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ndjson_batches_single_batch() {
        let records = vec![
            JsonValue::from("record1"),
            JsonValue::from("record2"),
            JsonValue::from("record3"),
        ];

        // Use "_test" to skip schema validation (no schema defined for this table)
        let batches = build_ndjson_batches(&records, 1024, "_test").unwrap();
        assert_eq!(batches.len(), 1);

        let body = String::from_utf8_lossy(&batches[0]);
        assert!(body.contains("record1"));
        assert!(body.contains("record2"));
        assert!(body.contains("record3"));
        // Verify NDJSON format (newline separated)
        assert_eq!(body.matches('\n').count(), 2);
    }

    #[test]
    fn build_ndjson_batches_splits_on_size() {
        let records = vec![
            JsonValue::from("aaaaaaaaaa"), // ~12 bytes with quotes
            JsonValue::from("bbbbbbbbbb"),
            JsonValue::from("cccccccccc"),
        ];

        // Force split with a small max size, use "_test" to skip schema validation
        let batches = build_ndjson_batches(&records, 30, "_test").unwrap();
        assert!(batches.len() > 1, "expected multiple batches");

        // Verify all records are present across batches
        let all_content: String = batches
            .iter()
            .map(|b| String::from_utf8_lossy(b).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all_content.contains("aaaaaaaaaa"));
        assert!(all_content.contains("bbbbbbbbbb"));
        assert!(all_content.contains("cccccccccc"));
    }

    #[test]
    fn build_ndjson_batches_always_includes_one_record() {
        // Even if a single record exceeds max size, it should still be included
        let records = vec![JsonValue::from(
            "this_is_a_very_long_record_that_exceeds_max",
        )];

        // Use "_test" to skip schema validation
        let batches = build_ndjson_batches(&records, 10, "_test").unwrap();
        assert_eq!(batches.len(), 1);
        assert!(String::from_utf8_lossy(&batches[0]).contains("this_is_a_very_long_record"));
    }

    // Schema validation tests are in crate::schema::tests

    #[test]
    fn validate_record_schema_catches_missing_field() {
        let json: JsonValue = serde_json::json!({
            "timestamp": 1234567890,
            "metric_name": "test.metric",
            "service_name": "test-service"
            // missing "value" field
        });

        let result = validate_record_schema(&json, "gauge", 0);
        assert!(result.is_err());
    }

    #[test]
    fn validate_record_schema_passes_valid_record() {
        let json: JsonValue = serde_json::json!({
            "timestamp": 1234567890,
            "value": 42.5,
            "metric_name": "test.metric",
            "service_name": "test-service"
        });

        let result = validate_record_schema(&json, "gauge", 0);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_record_schema_skips_unknown_tables() {
        // Unknown table names should pass through without validation
        let json: JsonValue = serde_json::json!({"anything": "goes"});
        let result = validate_record_schema(&json, "unknown_table", 0);
        assert!(result.is_ok());
    }

    #[test]
    fn send_error_retryable_classification() {
        assert!(SendError::Timeout.is_retryable());
        assert!(SendError::Network("conn reset".into()).is_retryable());
        assert!(SendError::Http {
            status: 502,
            endpoint: "x".into()
        }
        .is_retryable());
        assert!(SendError::Http {
            status: 503,
            endpoint: "x".into()
        }
        .is_retryable());
        assert!(SendError::Http {
            status: 504,
            endpoint: "x".into()
        }
        .is_retryable());
        assert!(!SendError::Serialize("bad json".into()).is_retryable());
        assert!(!SendError::Http {
            status: 400,
            endpoint: "x".into()
        }
        .is_retryable());
        assert!(!SendError::Http {
            status: 401,
            endpoint: "x".into()
        }
        .is_retryable());
        assert!(!SendError::Http {
            status: 500,
            endpoint: "x".into()
        }
        .is_retryable());
    }

    #[tokio::test]
    async fn missing_endpoint_reports_failure() {
        let client = PipelineClient::new(HashMap::new(), "token".to_string())
            .expect("failed to create client");
        let mut grouped = HashMap::new();
        grouped.insert("logs".to_string(), vec![JsonValue::from("test")]);

        let result = client.send_all(grouped).await;

        assert!(result.succeeded.is_empty());
        assert!(result.failed.contains_key("logs"));
    }
}
