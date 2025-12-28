//! Shared types for Durable Object requests and responses.

#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};

/// Query request for DO-level telemetry queries.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize, Serialize)]
pub struct DOQueryRequest {
    #[serde(default = "default_table")]
    pub table: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub trace_id: Option<String>,
    pub metric_name: Option<String>,
    #[serde(default)]
    pub labels: Vec<(String, String)>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[cfg(target_arch = "wasm32")]
fn default_table() -> String {
    "logs".to_string()
}

#[cfg(target_arch = "wasm32")]
fn default_limit() -> i64 {
    100
}

/// Count result from SQLite queries.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
pub struct CountRow {
    pub count: i64,
}
