//! HotCacheDO: Durable Object with SQLite storage for {service}:{signal} telemetry.

#[cfg(target_arch = "wasm32")]
use super::insert_helpers::{insert_gauge, insert_logs, insert_sum, insert_traces};
#[cfg(target_arch = "wasm32")]
use super::telemetry;
#[cfg(target_arch = "wasm32")]
use worker::*;
#[cfg(target_arch = "wasm32")]
include!(concat!(env!("OUT_DIR"), "/sqlite_ddl.rs"));

/// HotCacheDO: Durable Object with SQLite storage for recent telemetry data.
///
/// Each instance handles one {service}:{signal} combination.
/// Data is stored in SQLite tables (logs or traces) and cleaned up after retention period.
#[cfg(target_arch = "wasm32")]
#[durable_object]
pub struct HotCacheDO {
    state: State,
    env: Env,
}

#[cfg(target_arch = "wasm32")]
impl DurableObject for HotCacheDO {
    fn new(state: State, env: Env) -> Self {
        let do_instance = Self { state, env };

        // Initialize schema on creation - panic if schema init fails
        // as the DO cannot operate without proper tables
        if let Err(e) = do_instance.init_schema() {
            panic!("Failed to initialize SQLite schema: {}", e);
        }

        do_instance
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        match (req.method(), path) {
            (Method::Post, "/ingest") => self.handle_ingest(req).await,
            (_, "/query") => telemetry::handle_do_query(&self.state, req).await,
            (_, "/cleanup") => self.handle_cleanup().await,
            _ => Response::error("Not found", 404),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        // Triggered alarm - run cleanup
        self.handle_cleanup().await
    }
}

#[cfg(target_arch = "wasm32")]
impl HotCacheDO {
    /// Maximum retention period in seconds (7 days).
    /// Used to prevent integer overflow in alarm scheduling.
    const MAX_RETENTION_SECONDS: u64 = 7 * 24 * 3600; // 604800 seconds

    /// Initialize SQLite schema for logs and traces tables.
    fn init_schema(&self) -> Result<()> {
        let sql = self.state.storage().sql();

        // Logs table (generated)
        sql.exec(LOGS_DDL, None)?;

        // Index on timestamp for efficient cleanup and queries
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp)",
            None,
        )?;

        // Index on trace_id for trace lookups
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_logs_trace_id ON logs(trace_id)",
            None,
        )?;

        // Traces table (generated)
        sql.exec(TRACES_DDL, None)?;

        // Index on timestamp for efficient cleanup and queries
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_traces_timestamp ON traces(timestamp)",
            None,
        )?;

        // Index on trace_id for trace assembly
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_traces_trace_id ON traces(trace_id)",
            None,
        )?;

        // Gauge table (generated)
        sql.exec(GAUGE_DDL, None)?;

        // Index on timestamp for efficient cleanup and queries
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_gauge_timestamp ON gauge(timestamp)",
            None,
        )?;

        // Index on metric_name for metric lookups
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_gauge_metric_name ON gauge(metric_name)",
            None,
        )?;

        // Index on service_name for service filtering
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_gauge_service ON gauge(service_name)",
            None,
        )?;

        // Sum table (generated)
        sql.exec(SUM_DDL, None)?;

        // Index on timestamp for efficient cleanup and queries
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_sum_timestamp ON sum(timestamp)",
            None,
        )?;

        // Index on metric_name for metric lookups
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_sum_metric_name ON sum(metric_name)",
            None,
        )?;

        // Index on service_name for service filtering
        sql.exec(
            "CREATE INDEX IF NOT EXISTS idx_sum_service ON sum(service_name)",
            None,
        )?;

        Ok(())
    }

    /// Handle POST /ingest - receives JSON array of records from WasmHotCacheSender.
    async fn handle_ingest(&self, mut req: Request) -> Result<Response> {
        let body = req.text().await?;
        let records: Vec<serde_json::Value> = serde_json::from_str(&body)
            .map_err(|e| worker::Error::RustError(format!("Invalid JSON: {}", e)))?;

        if records.is_empty() {
            return Response::ok("0 records ingested");
        }

        // Determine table from first record's _table field
        let table = records
            .first()
            .and_then(|r| r.get("_table"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| worker::Error::RustError("Missing _table field".to_string()))?;

        let sql = self.state.storage().sql();
        let count = match table {
            "logs" => insert_logs(&sql, &records)?,
            "traces" => insert_traces(&sql, &records)?,
            "gauge" => insert_gauge(&sql, &records)?,
            "sum" => insert_sum(&sql, &records)?,
            _ => return Response::error(format!("Unknown table: {}", table), 400),
        };

        // Schedule cleanup alarm if not already set
        self.schedule_cleanup_alarm().await?;

        Response::ok(format!("{} records ingested", count))
    }

    /// Get clamped retention period in milliseconds.
    fn get_retention_ms(&self) -> u64 {
        let retention_seconds = self
            .env
            .var("HOT_CACHE_RETENTION_SECONDS")
            .map(|v| v.to_string().parse::<u64>().unwrap_or(3600))
            .unwrap_or(3600)
            .min(Self::MAX_RETENTION_SECONDS);
        retention_seconds * 1000
    }

    /// Handle GET /cleanup - delete old records and reschedule if needed.
    async fn handle_cleanup(&self) -> Result<Response> {
        let sql = self.state.storage().sql();

        let retention_ms = self.get_retention_ms();

        let now_ms = worker::Date::now().as_millis() as i64;
        // Use saturating_sub to prevent underflow
        let cutoff_ms = now_ms.saturating_sub(retention_ms as i64);

        // Delete old logs
        let logs_deleted = sql
            .exec(
                "DELETE FROM logs WHERE timestamp < ?",
                vec![SqlStorageValue::Integer(cutoff_ms)],
            )?
            .rows_written();

        // Delete old traces
        let traces_deleted = sql
            .exec(
                "DELETE FROM traces WHERE timestamp < ?",
                vec![SqlStorageValue::Integer(cutoff_ms)],
            )?
            .rows_written();

        // Delete old gauge metrics
        let gauge_deleted = sql
            .exec(
                "DELETE FROM gauge WHERE timestamp < ?",
                vec![SqlStorageValue::Integer(cutoff_ms)],
            )?
            .rows_written();

        // Delete old sum metrics
        let sum_deleted = sql
            .exec(
                "DELETE FROM sum WHERE timestamp < ?",
                vec![SqlStorageValue::Integer(cutoff_ms)],
            )?
            .rows_written();

        // Check if there are remaining records (propagate errors instead of defaulting to 0)
        let total_remaining = Self::get_table_count(&sql, "logs")?
            + Self::get_table_count(&sql, "traces")?
            + Self::get_table_count(&sql, "gauge")?
            + Self::get_table_count(&sql, "sum")?;

        // Reschedule alarm if records remain
        if total_remaining > 0 {
            self.schedule_cleanup_alarm().await?;
        } else {
            // No records left, clear alarm
            self.state.storage().delete_alarm().await?;
        }

        Response::ok(format!(
            "Deleted {} logs, {} traces, {} gauge, {} sum. {} records remaining.",
            logs_deleted, traces_deleted, gauge_deleted, sum_deleted, total_remaining
        ))
    }

    /// Schedule cleanup alarm for retention period from now.
    async fn schedule_cleanup_alarm(&self) -> Result<()> {
        // Check if alarm already exists - don't reschedule
        if self.state.storage().get_alarm().await?.is_some() {
            return Ok(());
        }

        // Safe: max 604800 * 1000 = 604_800_000 fits in u64 and i64
        let retention_ms = self.get_retention_ms();

        let now_ms = worker::Date::now().as_millis() as i64;
        // Use saturating_add to prevent overflow (would just schedule far future)
        let alarm_time_ms = now_ms.saturating_add(retention_ms as i64);

        self.state.storage().set_alarm(alarm_time_ms).await?;

        Ok(())
    }

    /// Safely extract count from SQL result, returning error instead of defaulting to 0.
    fn get_table_count(sql: &worker::SqlStorage, table: &str) -> Result<i64> {
        let query = format!("SELECT COUNT(*) as count FROM {}", table);
        let rows: Vec<CountRow> = sql.exec(&query, None)?.to_array().map_err(|e| {
            worker::Error::RustError(format!("Failed to count {} records: {}", table, e))
        })?;
        Ok(rows.first().map(|r| r.count).unwrap_or(0))
    }
}

// Re-export types from modules
#[cfg(target_arch = "wasm32")]
use super::types::CountRow;

/// Native placeholder - no implementation needed for tests.
#[cfg(not(target_arch = "wasm32"))]
pub struct HotCacheDO;
