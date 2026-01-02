//! AggregatorSender trait and implementations.

#[cfg(target_arch = "wasm32")]
use crate::convert::vrl_value_to_json_lossy;
#[cfg(target_arch = "wasm32")]
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use tracing::warn;
use vrl::value::{KeyString, Value};

/// Result of sending to aggregator DOs
#[derive(Debug, Default)]
pub struct AggregatorSendResult {
    pub succeeded: HashMap<String, usize>,
    pub failed: HashMap<String, String>,
}

/// Trait for sending records to AggregatorDO instances.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait AggregatorSender {
    async fn send_to_aggregator(
        &self,
        grouped: HashMap<String, Vec<Value>>,
    ) -> AggregatorSendResult;
}

/// WASM implementation that sends to AggregatorDO instances.
#[cfg(target_arch = "wasm32")]
pub struct WasmAggregatorSender {
    env: worker::Env,
    enabled: bool,
}

#[cfg(target_arch = "wasm32")]
impl WasmAggregatorSender {
    pub fn new(env: worker::Env) -> Self {
        let enabled = env
            .var("AGGREGATOR_ENABLED")
            .map(|v| v.to_string() == "true")
            .unwrap_or(false);

        Self { env, enabled }
    }

    /// Group records by DO name ({service}:{table}).
    /// Only processes logs and traces - metrics are skipped.
    fn group_by_do(&self, grouped: HashMap<String, Vec<Value>>) -> HashMap<String, Vec<Value>> {
        let mut by_do: HashMap<String, Vec<Value>> = HashMap::new();

        for (table_name, records) in grouped {
            // Skip metrics - they query from cold storage
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
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
impl AggregatorSender for WasmAggregatorSender {
    async fn send_to_aggregator(
        &self,
        grouped: HashMap<String, Vec<Value>>,
    ) -> AggregatorSendResult {
        if !self.enabled {
            // Aggregator disabled - return success without sending
            let succeeded: HashMap<String, usize> = grouped
                .into_iter()
                .filter(|(table, _)| table == "logs" || table == "traces")
                .map(|(table, records)| (table, records.len()))
                .collect();
            return AggregatorSendResult {
                succeeded,
                failed: HashMap::new(),
            };
        }

        let by_do = self.group_by_do(grouped);
        let mut succeeded = HashMap::new();
        let mut failed = HashMap::new();

        let results: Vec<_> = stream::iter(by_do)
            .map(|(do_name, records)| {
                let count = records.len();
                async move {
                    let result = self.send_to_do(&do_name, records).await;
                    (do_name, count, result)
                }
            })
            .buffer_unordered(10)
            .collect()
            .await;

        for (do_name, count, result) in results {
            match result {
                Ok(_) => {
                    *succeeded.entry(do_name).or_insert(0) += count;
                }
                Err(e) => {
                    tracing::warn!(do_name = %do_name, error = %e, "aggregator write failed");
                    failed.insert(do_name, e.to_string());
                }
            }
        }

        AggregatorSendResult { succeeded, failed }
    }
}

#[cfg(target_arch = "wasm32")]
impl WasmAggregatorSender {
    async fn send_to_do(&self, do_name: &str, records: Vec<Value>) -> Result<(), worker::Error> {
        let namespace = self.env.durable_object("AGGREGATOR")?;
        let id = namespace.id_from_name(do_name)?;
        let stub = id.get_stub()?;

        // Convert VRL Values to JSON for serialization
        let json_records: Vec<serde_json::Value> =
            records.iter().map(vrl_value_to_json_lossy).collect();

        let body = serde_json::to_string(&json_records)
            .map_err(|e| worker::Error::RustError(e.to_string()))?;

        // Extract signal from do_name (format: "service:signal")
        let signal = do_name.rsplit(':').next().unwrap_or("logs");
        let url = format!("http://do/ingest?signal={}", signal);

        let mut request = worker::Request::new_with_init(
            &url,
            worker::RequestInit::new()
                .with_method(worker::Method::Post)
                .with_body(Some(body.into())),
        )?;

        request
            .headers_mut()?
            .set("Content-Type", "application/json")?;

        let response = stub.fetch_with_request(request).await?;

        if response.status_code() >= 400 {
            return Err(worker::Error::RustError(format!(
                "DO returned status {}",
                response.status_code()
            )));
        }

        Ok(())
    }
}

/// NoOp implementation for native builds (testing).
/// In production WASM, this is replaced by WasmAggregatorSender.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeAggregatorSender;

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeAggregatorSender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeAggregatorSender {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl AggregatorSender for NativeAggregatorSender {
    async fn send_to_aggregator(
        &self,
        grouped: HashMap<String, Vec<Value>>,
    ) -> AggregatorSendResult {
        // NoOp for native - just count logs and traces
        let succeeded: HashMap<String, usize> = grouped
            .into_iter()
            .filter(|(table, _)| table == "logs" || table == "traces")
            .map(|(table, records)| (table, records.len()))
            .collect();

        AggregatorSendResult {
            succeeded,
            failed: HashMap::new(),
        }
    }
}

/// Extract service_name from record, defaulting to "unknown".
/// Validates that service names:
/// - Contain only alphanumeric characters, hyphens, underscores, or dots
/// - Are at most 128 characters long
/// - Are non-empty
///
/// Invalid service names are logged and replaced with "unknown" to prevent
/// conflicts with the `{service}:{signal}` DO naming scheme.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn get_service_name(record: &Value) -> String {
    if let Value::Object(ref map) = record {
        let key: KeyString = "service_name".into();
        if let Some(Value::Bytes(b)) = map.get(&key) {
            let s = std::str::from_utf8(b).unwrap_or("");
            // Validate: alphanumeric, hyphens, underscores, dots only
            // Max length 128 to prevent abuse
            if !s.is_empty()
                && s.len() <= 128
                && s.chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                return s.to_string();
            }
        }
    }
    warn!("Record missing or invalid service_name, routing to 'unknown'");
    "unknown".to_string()
}

/// Build DO name from service and table.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn build_do_name(service_name: &str, table_name: &str) -> String {
    format!("{}:{}", service_name, table_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vrl::value::Value;

    #[test]
    fn test_get_service_name_valid() {
        let record = Value::Object(
            [("service_name".into(), Value::from("payment-service"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "payment-service");
    }

    #[test]
    fn test_get_service_name_missing_returns_unknown() {
        let record = Value::Object(Default::default());
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_empty_returns_unknown() {
        let record = Value::Object(
            [("service_name".into(), Value::from(""))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_with_underscores_and_dots() {
        let record = Value::Object(
            [("service_name".into(), Value::from("payment_service.prod"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "payment_service.prod");
    }

    #[test]
    fn test_get_service_name_with_numbers() {
        let record = Value::Object(
            [("service_name".into(), Value::from("service123"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "service123");
    }

    #[test]
    fn test_get_service_name_with_colon_returns_unknown() {
        let record = Value::Object(
            [("service_name".into(), Value::from("payment:service"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_with_slash_returns_unknown() {
        let record = Value::Object(
            [("service_name".into(), Value::from("payment/service"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_with_special_chars_returns_unknown() {
        let record = Value::Object(
            [("service_name".into(), Value::from("payment@service#1"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_with_spaces_returns_unknown() {
        let record = Value::Object(
            [("service_name".into(), Value::from("payment service"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_exceeds_max_length_returns_unknown() {
        let long_name = "a".repeat(129);
        let record = Value::Object(
            [("service_name".into(), Value::from(long_name.as_str()))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), "unknown");
    }

    #[test]
    fn test_get_service_name_exactly_max_length() {
        let max_name = "a".repeat(128);
        let record = Value::Object(
            [("service_name".into(), Value::from(max_name.as_str()))]
                .into_iter()
                .collect(),
        );
        assert_eq!(get_service_name(&record), max_name);
    }

    #[test]
    fn test_build_do_name() {
        assert_eq!(
            build_do_name("payment-service", "logs"),
            "payment-service:logs"
        );
    }

    #[tokio::test]
    async fn test_native_sender_only_processes_logs_and_traces() {
        let sender = NativeAggregatorSender::new();
        let mut grouped = HashMap::new();
        grouped.insert(
            "logs".to_string(),
            vec![Value::Object(Default::default()); 5],
        );
        grouped.insert(
            "traces".to_string(),
            vec![Value::Object(Default::default()); 3],
        );
        grouped.insert(
            "gauge".to_string(),
            vec![Value::Object(Default::default()); 10],
        );

        let result = sender.send_to_aggregator(grouped).await;

        assert_eq!(result.succeeded.get("logs"), Some(&5));
        assert_eq!(result.succeeded.get("traces"), Some(&3));
        assert_eq!(result.succeeded.get("gauge"), None); // Metrics skipped
        assert!(result.failed.is_empty());
    }

    #[tokio::test]
    async fn test_native_sender_returns_success() {
        let sender = NativeAggregatorSender::new();
        let mut grouped = HashMap::new();
        grouped.insert(
            "logs".to_string(),
            vec![Value::Object(Default::default()); 5],
        );

        let result = sender.send_to_aggregator(grouped).await;

        assert_eq!(result.succeeded.get("logs"), Some(&5));
        assert!(result.failed.is_empty());
    }
}
