// src/registry/mod.rs
//! Service registry using Durable Objects for tracking known services.

pub mod cache;
pub mod sender;

#[cfg(target_arch = "wasm32")]
mod durable_object;

#[cfg(target_arch = "wasm32")]
pub use durable_object::{RegistryDO, ServiceRecord};
#[cfg(target_arch = "wasm32")]
pub use sender::WasmRegistrySender;

pub use sender::RegistrySender;

// Native placeholders for tests
#[cfg(not(target_arch = "wasm32"))]
pub struct RegistryDO;

#[cfg(not(target_arch = "wasm32"))]
pub use sender::NativeRegistrySender;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceRecord {
    pub name: String,
    pub first_seen_at: i64,
    pub has_logs: i64,
    pub has_traces: i64,
    pub has_metrics: i64,
}
