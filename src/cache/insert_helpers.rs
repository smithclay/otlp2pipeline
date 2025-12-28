//! Helper functions for inserting records into SQLite.
//!
//! This module provides a high-level API for batch inserting telemetry records
//! into the hot cache SQLite database. The actual SQL generation and value
//! extraction is done by generated code from VRL schemas.
//!
//! Note: Cloudflare DO SQLite automatically coalesces all writes within a single
//! request, so we don't need explicit transaction management.

#[cfg(target_arch = "wasm32")]
use worker::{Result, SqlStorage};

// Include generated insert helpers from build.rs
// These provide:
// - logs_insert_sql() / logs_values(record)
// - traces_insert_sql() / traces_values(record)
// - gauge_insert_sql() / gauge_values(record)
// - sum_insert_sql() / sum_values(record)
#[cfg(target_arch = "wasm32")]
include!(concat!(env!("OUT_DIR"), "/insert_helpers.rs"));

/// Insert log records into logs table.
///
/// All writes within a DO request are automatically atomic.
#[cfg(target_arch = "wasm32")]
pub fn insert_logs(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let insert_sql = logs_insert_sql();
    let mut count = 0;

    for record in records {
        let bindings = logs_values(record);
        sql.exec(insert_sql, bindings)?;
        count += 1;
    }

    Ok(count)
}

/// Insert trace records into traces table.
///
/// All writes within a DO request are automatically atomic.
#[cfg(target_arch = "wasm32")]
pub fn insert_traces(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let insert_sql = traces_insert_sql();
    let mut count = 0;

    for record in records {
        let bindings = traces_values(record);
        sql.exec(insert_sql, bindings)?;
        count += 1;
    }

    Ok(count)
}

/// Insert gauge metric records into gauge table.
///
/// All writes within a DO request are automatically atomic.
#[cfg(target_arch = "wasm32")]
pub fn insert_gauge(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let insert_sql = gauge_insert_sql();
    let mut count = 0;

    for record in records {
        let bindings = gauge_values(record);
        sql.exec(insert_sql, bindings)?;
        count += 1;
    }

    Ok(count)
}

/// Insert sum metric records into sum table.
///
/// All writes within a DO request are automatically atomic.
#[cfg(target_arch = "wasm32")]
pub fn insert_sum(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let insert_sql = sum_insert_sql();
    let mut count = 0;

    for record in records {
        let bindings = sum_values(record);
        sql.exec(insert_sql, bindings)?;
        count += 1;
    }

    Ok(count)
}
