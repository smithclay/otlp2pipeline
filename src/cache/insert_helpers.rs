//! Helper functions for inserting records into SQLite.
//!
//! This module provides a high-level API for batch inserting telemetry records
//! into the hot cache SQLite database. The actual SQL generation and value
//! extraction is done by generated code from VRL schemas.

#[cfg(target_arch = "wasm32")]
use worker::{Result, SqlStorage};

#[cfg(target_arch = "wasm32")]
use tracing::error;

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
/// This is a batch insert operation wrapped in a transaction.
/// All records are inserted or the entire batch is rolled back on error.
#[cfg(target_arch = "wasm32")]
pub fn insert_logs(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let mut count = 0;

    // Begin transaction for batch inserts
    sql.exec("BEGIN TRANSACTION", vec![])?;

    // Perform all inserts within the transaction
    let result = (|| -> Result<usize> {
        let insert_sql = logs_insert_sql();
        for record in records {
            let bindings = logs_values(record);
            sql.exec(insert_sql, bindings)?;
            count += 1;
        }
        Ok(count)
    })();

    // Commit on success, rollback on error
    match result {
        Ok(count) => {
            sql.exec("COMMIT", vec![])?;
            Ok(count)
        }
        Err(e) => {
            if let Err(rollback_err) = sql.exec("ROLLBACK", vec![]) {
                error!(
                    original_error = %e,
                    rollback_error = %rollback_err,
                    table = "logs",
                    "Transaction rollback failed after insert error"
                );
            }
            Err(e)
        }
    }
}

/// Insert trace records into traces table.
///
/// This is a batch insert operation wrapped in a transaction.
/// All records are inserted or the entire batch is rolled back on error.
#[cfg(target_arch = "wasm32")]
pub fn insert_traces(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let mut count = 0;

    // Begin transaction for batch inserts
    sql.exec("BEGIN TRANSACTION", vec![])?;

    // Perform all inserts within the transaction
    let result = (|| -> Result<usize> {
        let insert_sql = traces_insert_sql();
        for record in records {
            let bindings = traces_values(record);
            sql.exec(insert_sql, bindings)?;
            count += 1;
        }
        Ok(count)
    })();

    // Commit on success, rollback on error
    match result {
        Ok(count) => {
            sql.exec("COMMIT", vec![])?;
            Ok(count)
        }
        Err(e) => {
            if let Err(rollback_err) = sql.exec("ROLLBACK", vec![]) {
                error!(
                    original_error = %e,
                    rollback_error = %rollback_err,
                    table = "traces",
                    "Transaction rollback failed after insert error"
                );
            }
            Err(e)
        }
    }
}

/// Insert gauge metric records into gauge table.
///
/// This is a batch insert operation wrapped in a transaction.
/// All records are inserted or the entire batch is rolled back on error.
#[cfg(target_arch = "wasm32")]
pub fn insert_gauge(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let mut count = 0;

    // Begin transaction for batch inserts
    sql.exec("BEGIN TRANSACTION", vec![])?;

    // Perform all inserts within the transaction
    let result = (|| -> Result<usize> {
        let insert_sql = gauge_insert_sql();
        for record in records {
            let bindings = gauge_values(record);
            sql.exec(insert_sql, bindings)?;
            count += 1;
        }
        Ok(count)
    })();

    // Commit on success, rollback on error
    match result {
        Ok(count) => {
            sql.exec("COMMIT", vec![])?;
            Ok(count)
        }
        Err(e) => {
            if let Err(rollback_err) = sql.exec("ROLLBACK", vec![]) {
                error!(
                    original_error = %e,
                    rollback_error = %rollback_err,
                    table = "gauge",
                    "Transaction rollback failed after insert error"
                );
            }
            Err(e)
        }
    }
}

/// Insert sum metric records into sum table.
///
/// This is a batch insert operation wrapped in a transaction.
/// All records are inserted or the entire batch is rolled back on error.
#[cfg(target_arch = "wasm32")]
pub fn insert_sum(sql: &SqlStorage, records: &[serde_json::Value]) -> Result<usize> {
    let mut count = 0;

    // Begin transaction for batch inserts
    sql.exec("BEGIN TRANSACTION", vec![])?;

    // Perform all inserts within the transaction
    let result = (|| -> Result<usize> {
        let insert_sql = sum_insert_sql();
        for record in records {
            let bindings = sum_values(record);
            sql.exec(insert_sql, bindings)?;
            count += 1;
        }
        Ok(count)
    })();

    // Commit on success, rollback on error
    match result {
        Ok(count) => {
            sql.exec("COMMIT", vec![])?;
            Ok(count)
        }
        Err(e) => {
            if let Err(rollback_err) = sql.exec("ROLLBACK", vec![]) {
                error!(
                    original_error = %e,
                    rollback_error = %rollback_err,
                    table = "sum",
                    "Transaction rollback failed after insert error"
                );
            }
            Err(e)
        }
    }
}
