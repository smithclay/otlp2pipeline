//! AggregatorDO: Durable Object with SQLite storage for baseline stats.

#[cfg(target_arch = "wasm32")]
use super::stats::{LogAggregates, TraceAggregates};
#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use worker::*;

/// Signal type parsed from DO key.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AggregatorSignal {
    Logs,
    Traces,
}

#[cfg(target_arch = "wasm32")]
impl AggregatorSignal {
    fn from_key(key: &str) -> Self {
        match key.rsplit(':').next() {
            Some("traces") => AggregatorSignal::Traces,
            _ => AggregatorSignal::Logs, // Default to logs
        }
    }
}

/// Stats row for query responses.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsRow {
    pub minute: i64,
    pub count: i64,
    pub error_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_sum_us: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_min_us: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_max_us: Option<i64>,
}

/// AggregatorDO: Stores per-minute aggregate stats for logs or traces.
#[cfg(target_arch = "wasm32")]
#[durable_object]
pub struct AggregatorDO {
    state: State,
    env: Env,
    signal: AggregatorSignal,
}

#[cfg(target_arch = "wasm32")]
impl DurableObject for AggregatorDO {
    fn new(state: State, env: Env) -> Self {
        // Parse signal from DO id name (e.g., "my-service:logs")
        let id_name = state.id().name().unwrap_or_default();
        let signal = AggregatorSignal::from_key(&id_name);
        let do_instance = Self { state, env, signal };

        // Initialize schema on creation
        if let Err(e) = do_instance.ensure_schema() {
            panic!("Failed to initialize SQLite schema: {}", e);
        }

        do_instance
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();
        match (req.method(), path.as_str()) {
            (Method::Post, "/ingest") => self.handle_ingest(req).await,
            (Method::Get, "/stats") => self.handle_stats_query(req).await,
            _ => Response::error("Not found", 404),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        self.handle_cleanup().await
    }
}

#[cfg(target_arch = "wasm32")]
impl AggregatorDO {
    const LOGS_DDL: &'static str = "CREATE TABLE IF NOT EXISTS stats (
        minute INTEGER PRIMARY KEY,
        count INTEGER DEFAULT 0,
        error_count INTEGER DEFAULT 0
    )";

    const TRACES_DDL: &'static str = "CREATE TABLE IF NOT EXISTS stats (
        minute INTEGER PRIMARY KEY,
        count INTEGER DEFAULT 0,
        error_count INTEGER DEFAULT 0,
        latency_sum_us INTEGER DEFAULT 0,
        latency_min_us INTEGER,
        latency_max_us INTEGER
    )";

    fn ensure_schema(&self) -> Result<()> {
        let ddl = match self.signal {
            AggregatorSignal::Logs => Self::LOGS_DDL,
            AggregatorSignal::Traces => Self::TRACES_DDL,
        };
        self.state.storage().sql().exec(ddl, None)?;
        Ok(())
    }

    fn now_minute() -> i64 {
        let now_ms = worker::Date::now().as_millis() as i64;
        now_ms / 60_000 // Convert to minutes
    }

    async fn handle_ingest(&self, mut req: Request) -> Result<Response> {
        let body = req.text().await?;
        let records: Vec<serde_json::Value> = serde_json::from_str(&body)
            .map_err(|e| worker::Error::RustError(format!("Invalid JSON: {}", e)))?;

        if records.is_empty() {
            return Response::ok("0");
        }

        let minute = Self::now_minute();

        match self.signal {
            AggregatorSignal::Logs => {
                let mut agg = LogAggregates::default();
                for record in &records {
                    agg.accumulate(record);
                }
                self.upsert_log_stats(minute, &agg)?;
            }
            AggregatorSignal::Traces => {
                let mut agg = TraceAggregates::default();
                for record in &records {
                    agg.accumulate(record);
                }
                self.upsert_trace_stats(minute, &agg)?;
            }
        }

        // Schedule cleanup alarm if not already set
        self.schedule_cleanup_alarm().await?;

        Response::ok(format!("{}", records.len()))
    }

    fn upsert_log_stats(&self, minute: i64, stats: &LogAggregates) -> Result<()> {
        let sql = self.state.storage().sql();
        sql.exec(
            "INSERT INTO stats (minute, count, error_count) VALUES (?, ?, ?)
             ON CONFLICT(minute) DO UPDATE SET
               count = count + excluded.count,
               error_count = error_count + excluded.error_count",
            vec![
                SqlStorageValue::Integer(minute),
                SqlStorageValue::Integer(stats.count),
                SqlStorageValue::Integer(stats.error_count),
            ],
        )?;
        Ok(())
    }

    fn upsert_trace_stats(&self, minute: i64, stats: &TraceAggregates) -> Result<()> {
        let sql = self.state.storage().sql();
        sql.exec(
            "INSERT INTO stats (minute, count, error_count, latency_sum_us, latency_min_us, latency_max_us)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(minute) DO UPDATE SET
               count = count + excluded.count,
               error_count = error_count + excluded.error_count,
               latency_sum_us = latency_sum_us + excluded.latency_sum_us,
               latency_min_us = MIN(COALESCE(latency_min_us, excluded.latency_min_us), excluded.latency_min_us),
               latency_max_us = MAX(COALESCE(latency_max_us, excluded.latency_max_us), excluded.latency_max_us)",
            vec![
                SqlStorageValue::Integer(minute),
                SqlStorageValue::Integer(stats.count),
                SqlStorageValue::Integer(stats.error_count),
                SqlStorageValue::Integer(stats.latency_sum_us),
                stats.latency_min_us.map(SqlStorageValue::Integer).unwrap_or(SqlStorageValue::Null),
                stats.latency_max_us.map(SqlStorageValue::Integer).unwrap_or(SqlStorageValue::Null),
            ],
        )?;
        Ok(())
    }

    async fn handle_stats_query(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let from = params.get("from").and_then(|v| v.parse::<i64>().ok());
        let to = params.get("to").and_then(|v| v.parse::<i64>().ok());

        let mut query = "SELECT * FROM stats WHERE 1=1".to_string();
        let mut binds: Vec<SqlStorageValue> = vec![];

        if let Some(from) = from {
            query.push_str(" AND minute >= ?");
            binds.push(SqlStorageValue::Integer(from));
        }
        if let Some(to) = to {
            query.push_str(" AND minute <= ?");
            binds.push(SqlStorageValue::Integer(to));
        }
        query.push_str(" ORDER BY minute");

        let sql = self.state.storage().sql();
        let result = if !binds.is_empty() {
            sql.exec(&query, Some(binds))?
        } else {
            sql.exec(&query, None)?
        };

        let rows: Vec<StatsRow> = result.to_array().map_err(|e| {
            worker::Error::RustError(format!("Failed to deserialize stats rows: {}", e))
        })?;

        Response::from_json(&rows)
    }

    const MAX_RETENTION_MINUTES: i64 = 10080; // 7 days

    fn get_retention_minutes(&self) -> i64 {
        self.env
            .var("AGGREGATOR_RETENTION_MINUTES")
            .map(|v| v.to_string().parse::<i64>().unwrap_or(60))
            .unwrap_or(60)
            .min(Self::MAX_RETENTION_MINUTES)
    }

    async fn handle_cleanup(&self) -> Result<Response> {
        let retention = self.get_retention_minutes();
        let cutoff = Self::now_minute().saturating_sub(retention);

        let sql = self.state.storage().sql();
        let deleted = sql
            .exec(
                "DELETE FROM stats WHERE minute < ?",
                vec![SqlStorageValue::Integer(cutoff)],
            )?
            .rows_written();

        // Check if any records remain
        let remaining = self.get_stats_count()?;

        // Reschedule alarm if records remain
        if remaining > 0 {
            self.schedule_cleanup_alarm().await?;
        } else {
            // No records left, clear alarm
            self.state.storage().delete_alarm().await?;
        }

        Response::ok(format!(
            "Deleted {} stats. {} remaining.",
            deleted, remaining
        ))
    }

    async fn schedule_cleanup_alarm(&self) -> Result<()> {
        if self.state.storage().get_alarm().await?.is_some() {
            return Ok(());
        }

        let now_ms = worker::Date::now().as_millis() as i64;
        let alarm_time_ms = now_ms.saturating_add(60_000); // 1 minute from now
        self.state.storage().set_alarm(alarm_time_ms).await?;
        Ok(())
    }

    fn get_stats_count(&self) -> Result<i64> {
        let sql = self.state.storage().sql();
        let rows: Vec<CountRow> = sql
            .exec("SELECT COUNT(*) as count FROM stats", None)?
            .to_array()
            .map_err(|e| {
                worker::Error::RustError(format!("Failed to count stats records: {}", e))
            })?;
        Ok(rows.first().map(|r| r.count).unwrap_or(0))
    }
}

/// Helper type for COUNT queries.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}
