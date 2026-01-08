//! RegistrySender trait and implementations.

use crate::signal::Signal;

#[cfg(target_arch = "wasm32")]
use super::cache;
#[cfg(target_arch = "wasm32")]
use super::durable_object::{RegisterRequest, ServiceRecord, ServiceRegistration};

#[cfg(not(target_arch = "wasm32"))]
use super::ServiceRecord;

/// Trait for registering services with the RegistryDO.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait RegistrySender {
    /// Register services (fire-and-forget - used via ctx.wait_until).
    async fn register_services(&self, services: Vec<String>, signal: Signal) -> Result<(), String>;

    /// Get all services (for API endpoint).
    async fn get_all_services(&self) -> Result<Vec<ServiceRecord>, String>;
}

/// WASM implementation that uses local cache and sends to RegistryDO.
#[cfg(target_arch = "wasm32")]
pub struct WasmRegistrySender {
    env: worker::Env,
}

#[cfg(target_arch = "wasm32")]
impl WasmRegistrySender {
    pub fn new(env: worker::Env) -> Self {
        Self { env }
    }

    /// Get the RegistryDO stub.
    fn get_stub(&self) -> Result<worker::Stub, worker::Error> {
        let namespace = self.env.durable_object("REGISTRY")?;
        let id = namespace.id_from_name("services-registry")?;
        id.get_stub()
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
impl RegistrySender for WasmRegistrySender {
    async fn register_services(&self, services: Vec<String>, signal: Signal) -> Result<(), String> {
        // Map signal to registry categories (logs, traces, metrics)
        // All metric signal types (Gauge, Sum, Histogram, etc.) map to "metrics"
        let signal_name = match signal {
            Signal::Logs => "logs",
            Signal::Traces => "traces",
            Signal::Gauge
            | Signal::Sum
            | Signal::Histogram
            | Signal::ExpHistogram
            | Signal::Summary => "metrics",
        };

        // Filter to only new (service, signal) combinations not in local cache
        let new_services: Vec<String> = services
            .into_iter()
            .filter(|name| !cache::is_known(name, signal_name))
            .collect();

        if new_services.is_empty() {
            return Ok(());
        }

        // Send to DO for persistence first
        let stub = self
            .get_stub()
            .map_err(|e| format!("Failed to get RegistryDO stub: {}", e))?;

        let registrations: Vec<ServiceRegistration> = new_services
            .iter()
            .map(|name| ServiceRegistration {
                name: name.clone(),
                signal: signal_name.to_string(),
            })
            .collect();

        let request_body = RegisterRequest {
            services: registrations,
        };

        let body = serde_json::to_string(&request_body)
            .map_err(|e| format!("Failed to serialize registration request: {}", e))?;

        let mut request = worker::Request::new_with_init(
            "http://do/register",
            worker::RequestInit::new()
                .with_method(worker::Method::Post)
                .with_body(Some(body.into())),
        )
        .map_err(|e| format!("Failed to create request: {}", e))?;

        request
            .headers_mut()
            .map_err(|e| format!("Failed to get request headers: {}", e))?
            .set("Content-Type", "application/json")
            .map_err(|e| format!("Failed to set Content-Type header: {}", e))?;

        let response = stub
            .fetch_with_request(request)
            .await
            .map_err(|e| format!("Failed to send to RegistryDO: {}", e))?;

        if response.status_code() >= 400 {
            return Err(format!(
                "RegistryDO returned status {}",
                response.status_code()
            ));
        }

        // Only add to local cache after successful DO write
        for service in &new_services {
            cache::add_locally(service.clone(), signal_name.to_string());
        }

        Ok(())
    }

    async fn get_all_services(&self) -> Result<Vec<ServiceRecord>, String> {
        // Note: We always query the DO because the cache only stores service names,
        // not full ServiceRecord objects with metadata (first_seen_at, has_logs, etc.)
        // Query DO for full service records
        let stub = self
            .get_stub()
            .map_err(|e| format!("Failed to get RegistryDO stub: {}", e))?;

        let request = worker::Request::new_with_init(
            "http://do/list",
            worker::RequestInit::new().with_method(worker::Method::Get),
        )
        .map_err(|e| format!("Failed to create request: {}", e))?;

        let mut response = stub
            .fetch_with_request(request)
            .await
            .map_err(|e| format!("Failed to fetch from RegistryDO: {}", e))?;

        if response.status_code() >= 400 {
            return Err(format!(
                "RegistryDO returned status {}",
                response.status_code()
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        let services: Vec<ServiceRecord> = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse service records: {}", e))?;

        // Refresh cache with service names
        let service_names: Vec<String> = services.iter().map(|s| s.name.clone()).collect();
        cache::refresh(service_names);

        Ok(services)
    }
}

/// NoOp implementation for native builds (testing).
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeRegistrySender;

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeRegistrySender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeRegistrySender {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl RegistrySender for NativeRegistrySender {
    async fn register_services(
        &self,
        _services: Vec<String>,
        _signal: Signal,
    ) -> Result<(), String> {
        // NoOp for native
        Ok(())
    }

    async fn get_all_services(&self) -> Result<Vec<ServiceRecord>, String> {
        // NoOp for native - return empty list
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_native_sender_register_returns_ok() {
        let sender = NativeRegistrySender::new();
        let result = sender
            .register_services(vec!["service1".to_string()], Signal::Logs)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_native_sender_get_all_returns_empty() {
        let sender = NativeRegistrySender::new();
        let result = sender.get_all_services().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}
