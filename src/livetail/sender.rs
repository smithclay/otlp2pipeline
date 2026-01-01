//! LiveTailSender trait and implementations.

use std::collections::HashMap;
use vrl::value::Value;

/// Result of sending to livetail DOs.
#[derive(Debug, Default)]
pub struct LiveTailSendResult {
    /// Number of records sent per DO.
    pub sent: HashMap<String, usize>,
    /// Errors per DO (best-effort, logged but not fatal).
    pub errors: HashMap<String, String>,
}

impl LiveTailSendResult {
    /// Create a disabled result (feature flag off).
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Create a success result.
    pub fn ok() -> Self {
        Self::default()
    }
}

/// Trait for sending records to LiveTailDO instances.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait LiveTailSender {
    /// Send grouped records to relevant LiveTailDOs.
    ///
    /// Records are grouped by table name (logs, traces).
    /// Each record contains service_name for DO routing.
    async fn send_to_livetail(&self, grouped: HashMap<String, Vec<Value>>) -> LiveTailSendResult;
}

/// NoOp implementation for native builds (testing).
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeLiveTailSender;

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeLiveTailSender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeLiveTailSender {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl LiveTailSender for NativeLiveTailSender {
    async fn send_to_livetail(&self, _grouped: HashMap<String, Vec<Value>>) -> LiveTailSendResult {
        // NoOp for native - livetail is a WASM-only feature
        LiveTailSendResult::disabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_result_defaults() {
        let result = LiveTailSendResult::default();
        assert!(result.sent.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_send_result_disabled() {
        let result = LiveTailSendResult::disabled();
        assert!(result.sent.is_empty());
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_native_sender_returns_disabled() {
        let sender = NativeLiveTailSender::new();
        let result = sender.send_to_livetail(HashMap::new()).await;
        assert!(result.sent.is_empty());
        assert!(result.errors.is_empty());
    }
}

// === WASM Implementation ===

#[cfg(target_arch = "wasm32")]
use crate::aggregator::{build_do_name, get_service_name};
#[cfg(target_arch = "wasm32")]
use crate::convert::vrl_value_to_json_lossy;
#[cfg(target_arch = "wasm32")]
use crate::livetail::cache;
#[cfg(target_arch = "wasm32")]
use futures::stream::{self, StreamExt};

/// WASM implementation that sends to LiveTailDO instances.
#[cfg(target_arch = "wasm32")]
pub struct WasmLiveTailSender {
    env: worker::Env,
    enabled: bool,
}

#[cfg(target_arch = "wasm32")]
impl WasmLiveTailSender {
    pub fn new(env: worker::Env) -> Self {
        let enabled = env
            .var("LIVETAIL_ENABLED")
            .map(|v| v.to_string() == "true")
            .unwrap_or(false);

        Self { env, enabled }
    }

    /// Group records by DO name ({service}:{table}).
    /// Only processes logs and traces - metrics are skipped.
    fn group_by_do(&self, grouped: HashMap<String, Vec<Value>>) -> HashMap<String, Vec<Value>> {
        let mut by_do: HashMap<String, Vec<Value>> = HashMap::new();

        for (table_name, records) in grouped {
            // Skip metrics - live tail is only for logs and traces
            if table_name != "logs" && table_name != "traces" {
                continue;
            }

            for record in records {
                let service = get_service_name(&record);
                let do_name = build_do_name(&service, &table_name);
                by_do.entry(do_name).or_default().push(record);
            }
        }

        by_do
    }

    /// Send records to a single DO, return client count.
    async fn send_to_do(&self, do_name: &str, records: Vec<Value>) -> Result<usize, worker::Error> {
        let namespace = self.env.durable_object("LIVETAIL")?;
        let id = namespace.id_from_name(do_name)?;
        let stub = id.get_stub()?;

        // Convert VRL Values to JSON for serialization
        let json_records: Vec<serde_json::Value> =
            records.iter().map(vrl_value_to_json_lossy).collect();

        let body = serde_json::to_string(&json_records)
            .map_err(|e| worker::Error::RustError(e.to_string()))?;

        let mut request = worker::Request::new_with_init(
            "http://do/ingest",
            worker::RequestInit::new()
                .with_method(worker::Method::Post)
                .with_body(Some(body.into())),
        )?;

        request
            .headers_mut()?
            .set("Content-Type", "application/json")?;

        let mut response = stub.fetch_with_request(request).await?;

        if response.status_code() >= 400 {
            return Err(worker::Error::RustError(format!(
                "DO returned status {}",
                response.status_code()
            )));
        }

        // Parse client count from response
        let count_str = response.text().await?;
        let client_count = count_str.parse::<usize>().unwrap_or(0);

        Ok(client_count)
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
impl LiveTailSender for WasmLiveTailSender {
    async fn send_to_livetail(&self, grouped: HashMap<String, Vec<Value>>) -> LiveTailSendResult {
        if !self.enabled {
            return LiveTailSendResult::disabled();
        }

        let by_do = self.group_by_do(grouped);
        let mut result = LiveTailSendResult::default();

        // Process DOs concurrently (max 10 at a time)
        let results: Vec<_> = stream::iter(by_do)
            .map(|(do_name, records)| {
                let count = records.len();
                async move {
                    // Check cache first
                    match cache::has_clients(&do_name) {
                        Some(false) => {
                            // Cached: no clients, skip DO call
                            (do_name, count, Ok(0_usize))
                        }
                        Some(true) => {
                            // Cached: has clients, send records
                            let res = self.send_to_do(&do_name, records).await;
                            (do_name, count, res)
                        }
                        None => {
                            // Cache miss/stale, call DO to refresh
                            let res = self.send_to_do(&do_name, records).await;
                            // Update cache with result
                            if let Ok(client_count) = &res {
                                cache::update(&do_name, *client_count > 0);
                            }
                            (do_name, count, res)
                        }
                    }
                }
            })
            .buffer_unordered(10)
            .collect()
            .await;

        for (do_name, count, res) in results {
            match res {
                Ok(client_count) if client_count > 0 => {
                    *result.sent.entry(do_name).or_insert(0) += count;
                }
                Ok(_) => {
                    // No clients - records not sent (expected)
                }
                Err(e) => {
                    tracing::debug!(do_name = %do_name, error = %e, "livetail send skipped");
                    result.errors.insert(do_name, e.to_string());
                }
            }
        }

        result
    }
}
