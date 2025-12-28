//! Hot cache layer using Durable Objects with SQLite storage.
//!
//! Provides immediate query access to recent telemetry while the pipeline
//! processes data into R2/Iceberg (5-10 minute delay).

mod durable_object;
mod insert_helpers;
pub mod types;

pub mod parquet;
pub mod telemetry;

// Re-exports for backwards compatibility
#[allow(unused_imports)]
pub use durable_object::HotCacheDO;
#[allow(unused_imports)]
pub use telemetry::HotCacheSender;
#[cfg(target_arch = "wasm32")]
#[allow(unused_imports)]
pub use telemetry::WasmHotCacheSender;

// Direct module declaration (moved from telemetry to cache root)
pub mod arrow_convert;
pub use telemetry::sender;
