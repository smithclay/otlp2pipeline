//! HotCacheSender trait and implementations.

use crate::pipeline::SendResult;
use std::collections::HashMap;
use tracing::warn;
use vrl::value::{KeyString, Value};

// Trait only used on WASM (handle_signal_with_cache); native uses handle_signal directly
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait HotCacheSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult;
}

/// WASM implementation that sends to Durable Objects.
#[cfg(target_arch = "wasm32")]
pub struct WasmHotCacheSender {
    env: worker::Env,
    enabled: bool,
}

#[cfg(target_arch = "wasm32")]
impl WasmHotCacheSender {
    pub fn new(env: worker::Env) -> Self {
        let enabled = env
            .var("HOT_CACHE_ENABLED")
            .map(|v| v.to_string() == "true")
            .unwrap_or(false);

        Self { env, enabled }
    }

    /// Group records by DO name ({service}:{table}).
    fn group_by_do(&self, grouped: HashMap<String, Vec<Value>>) -> HashMap<String, Vec<Value>> {
        let mut by_do: HashMap<String, Vec<Value>> = HashMap::new();

        for (table_name, records) in grouped {
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
impl HotCacheSender for WasmHotCacheSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult {
        if !self.enabled {
            // Hot cache disabled - return success without sending
            let succeeded: HashMap<String, usize> = grouped
                .into_iter()
                .map(|(table, records)| (table, records.len()))
                .collect();
            return SendResult {
                succeeded,
                failed: HashMap::new(),
            };
        }

        let by_do = self.group_by_do(grouped);
        let mut succeeded = HashMap::new();
        let mut failed = HashMap::new();

        for (do_name, records) in by_do {
            let count = records.len();
            match self.send_to_do(&do_name, records).await {
                Ok(_) => {
                    *succeeded.entry(do_name).or_insert(0) += count;
                }
                Err(e) => {
                    tracing::warn!(do_name = %do_name, error = %e, "hot cache write failed");
                    failed.insert(do_name, e.to_string());
                }
            }
        }

        SendResult { succeeded, failed }
    }
}

#[cfg(target_arch = "wasm32")]
impl WasmHotCacheSender {
    async fn send_to_do(&self, do_name: &str, records: Vec<Value>) -> Result<(), worker::Error> {
        let namespace = self.env.durable_object("HOT_CACHE")?;
        let id = namespace.id_from_name(do_name)?;
        let stub = id.get_stub()?;

        let body =
            serde_json::to_string(&records).map_err(|e| worker::Error::RustError(e.to_string()))?;

        let mut request = worker::Request::new_with_init(
            "http://do/ingest",
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
/// In production WASM, this is replaced by WasmHotCacheSender.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeHotCacheSender;

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeHotCacheSender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeHotCacheSender {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl HotCacheSender for NativeHotCacheSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult {
        // NoOp for native - just count records
        let succeeded: HashMap<String, usize> = grouped
            .into_iter()
            .map(|(table, records)| (table, records.len()))
            .collect();

        SendResult {
            succeeded,
            failed: HashMap::new(),
        }
    }
}

/// Extract service_name from record, defaulting to "unknown".
pub fn get_service_name(record: &Value) -> String {
    if let Value::Object(ref map) = record {
        let key: KeyString = "service_name".into();
        if let Some(Value::Bytes(b)) = map.get(&key) {
            let s = std::str::from_utf8(b).unwrap_or("");
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    warn!("Record missing service_name, routing to 'unknown'");
    "unknown".to_string()
}

/// Build DO name from service and table.
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
    fn test_build_do_name() {
        assert_eq!(
            build_do_name("payment-service", "logs"),
            "payment-service:logs"
        );
    }

    #[tokio::test]
    async fn test_noop_sender_returns_success() {
        let sender = NativeHotCacheSender::new();
        let mut grouped = HashMap::new();
        grouped.insert(
            "logs".to_string(),
            vec![Value::Object(Default::default()); 5],
        );

        let result = sender.send_all(grouped).await;

        assert_eq!(result.succeeded.get("logs"), Some(&5));
        assert!(result.failed.is_empty());
    }
}
