//! Telemetry domain: logs and traces query handling.

pub mod do_query;
pub mod sender;

// Re-export from cache root for backwards compatibility
pub use super::arrow_convert;

// Re-exports for public API
#[cfg(target_arch = "wasm32")]
pub use do_query::handle_do_query;
pub use sender::HotCacheSender;
#[cfg(target_arch = "wasm32")]
pub use sender::WasmHotCacheSender;
