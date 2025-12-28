//! Parquet export endpoints for DuckDB httpfs compatibility.
//!
//! Provides GET endpoints that return Parquet files:
//! - `/logs` - Log records
//! - `/traces` - Span records
//! - `/metrics/gauge` - Gauge metrics
//! - `/metrics/sum` - Sum/counter metrics

mod convert;
mod params;

#[cfg(target_arch = "wasm32")]
mod export;

pub use convert::write_parquet;
pub use params::{ExportError, ExportParams};

#[cfg(target_arch = "wasm32")]
pub use export::{
    handle_gauge_export, handle_logs_export, handle_sum_export, handle_traces_export,
};
