//! Parquet export endpoint handlers.
//!
//! WASM-only - these handlers run in Cloudflare Workers.

use futures::future::join_all;
use serde_json::Value;
use tracing::warn;
use worker::{Env, Method, Request, RequestInit, Response, Result};

use super::convert::write_parquet;
use super::params::{ExportError, ExportParams};
use crate::cache::arrow_convert::{
    json_to_gauge_batch, json_to_logs_batch, json_to_sum_batch, json_to_traces_batch,
};

/// Content type for Parquet responses.
const PARQUET_CONTENT_TYPE: &str = "application/vnd.apache.parquet";

/// Handle GET /logs endpoint.
pub async fn handle_logs_export(req: Request, env: Env) -> Result<Response> {
    handle_export(req, env, "logs", |rows| {
        json_to_logs_batch(rows).map_err(|e| ExportError::bad_request(e.to_string()))
    })
    .await
}

/// Handle GET /traces endpoint.
pub async fn handle_traces_export(req: Request, env: Env) -> Result<Response> {
    handle_export(req, env, "traces", |rows| {
        json_to_traces_batch(rows).map_err(|e| ExportError::bad_request(e.to_string()))
    })
    .await
}

/// Handle GET /metrics/gauge endpoint.
pub async fn handle_gauge_export(req: Request, env: Env) -> Result<Response> {
    handle_export(req, env, "gauge", |rows| {
        json_to_gauge_batch(rows).map_err(|e| ExportError::bad_request(e.to_string()))
    })
    .await
}

/// Handle GET /metrics/sum endpoint.
pub async fn handle_sum_export(req: Request, env: Env) -> Result<Response> {
    handle_export(req, env, "sum", |rows| {
        json_to_sum_batch(rows).map_err(|e| ExportError::bad_request(e.to_string()))
    })
    .await
}

/// Generic export handler for any signal type.
async fn handle_export<F>(req: Request, env: Env, signal: &str, to_batch: F) -> Result<Response>
where
    F: Fn(&[Value]) -> std::result::Result<arrow_array::RecordBatch, ExportError>,
{
    // Parse and validate parameters
    let query = req.url()?.query().unwrap_or_default().to_string();
    let params = match ExportParams::from_query_string(&query) {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };

    if let Err(e) = params.validate() {
        return error_response(&e);
    }

    // Fan out to DOs
    let do_names = params.do_names(signal);
    let (rows, partial, failed_sources) = fanout_query(&env, &do_names, &params, signal).await?;

    // Handle no data case
    if rows.is_empty() && !partial {
        return error_response(&ExportError::not_found("No data found"));
    }

    // Convert to Arrow batch
    let batch = match to_batch(&rows) {
        Ok(b) => b,
        Err(e) => return error_response(&e),
    };

    // Convert to Parquet
    let parquet_bytes = match write_parquet(&batch) {
        Ok(b) => b,
        Err(e) => return error_response(&ExportError::bad_request(e.to_string())),
    };

    // Build response
    let status_code = if partial { 206 } else { 200 };
    let headers = worker::Headers::new();
    headers.set("Content-Type", PARQUET_CONTENT_TYPE)?;
    headers.set("X-Query-Partial", &partial.to_string())?;
    headers.set("X-Query-Row-Count", &rows.len().to_string())?;

    if !failed_sources.is_empty() {
        headers.set("X-Query-Failed-Sources", &failed_sources.join(","))?;
    }

    // Add Content-Disposition for download
    let filename = format!("{}-export.parquet", signal);
    headers.set(
        "Content-Disposition",
        &format!("attachment; filename=\"{}\"", filename),
    )?;

    Response::from_bytes(parquet_bytes).map(|r| r.with_headers(headers).with_status(status_code))
}

/// Fan out query to multiple DOs and aggregate results.
///
/// Returns: (rows, partial, failed_sources)
async fn fanout_query(
    env: &Env,
    do_names: &[String],
    params: &ExportParams,
    signal: &str,
) -> Result<(Vec<Value>, bool, Vec<String>)> {
    let namespace = env.durable_object("HOT_CACHE")?;

    // Build query payload for DOs
    let table = signal.to_string();
    let do_query = serde_json::json!({
        "table": table,
        "start_time": params.start_ms(),
        "end_time": params.end_ms(),
        "trace_id": params.trace_id,
        "metric_name": params.metric_name,
        "labels": params.labels,
        "limit": params.limit,
    });

    // Create futures for parallel DO queries
    let fetch_futures: Vec<_> = do_names
        .iter()
        .map(|do_name| {
            let do_name = do_name.clone();
            let namespace = namespace.clone();
            let query = do_query.clone();

            async move { query_single_do(&namespace, &do_name, &query).await }
        })
        .collect();

    // Execute all DO fetches in parallel
    let results = join_all(fetch_futures).await;

    // Aggregate results
    let mut all_rows: Vec<Value> = Vec::new();
    let mut failed_sources: Vec<String> = Vec::new();

    for (do_name, result) in do_names.iter().zip(results.into_iter()) {
        match result {
            Ok(rows) => all_rows.extend(rows),
            Err(e) => {
                warn!(
                    do_name = %do_name,
                    error = %e,
                    "Durable Object query failed during fanout"
                );
                failed_sources.push(do_name.clone());
            }
        }
    }

    // Sort by timestamp descending with deterministic tiebreakers
    sort_records_by_timestamp_desc(&mut all_rows);

    // Apply limit
    all_rows.truncate(params.limit);

    let partial = !failed_sources.is_empty();

    Ok((all_rows, partial, failed_sources))
}

/// Query a single DO instance.
async fn query_single_do(
    namespace: &worker::ObjectNamespace,
    do_name: &str,
    query: &Value,
) -> Result<Vec<Value>> {
    let id = namespace.id_from_name(do_name)?;
    let stub = id.get_stub()?;

    let body = serde_json::to_string(&query).map_err(|e| format!("serialize failed: {}", e))?;

    let mut do_req = Request::new_with_init(
        "http://do/query",
        RequestInit::new()
            .with_method(Method::Post)
            .with_body(Some(body.into())),
    )
    .map_err(|e| format!("request init failed: {}", e))?;

    do_req
        .headers_mut()
        .map_err(|e| format!("headers_mut failed: {}", e))?
        .set("Content-Type", "application/json")
        .map_err(|e| format!("set header failed: {}", e))?;

    let mut resp = stub
        .fetch_with_request(do_req)
        .await
        .map_err(|e| format!("fetch failed: {}", e))?;

    if resp.status_code() >= 400 {
        return Err(format!("status {}", resp.status_code()).into());
    }

    resp.json::<Vec<Value>>()
        .await
        .map_err(|e| format!("json decode failed: {}", e).into())
}

/// Sort records by timestamp (descending) with deterministic tiebreakers.
fn sort_records_by_timestamp_desc(data: &mut [Value]) {
    data.sort_by(|a, b| {
        let ts_a = a.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
        let ts_b = b.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
        match ts_b.cmp(&ts_a) {
            std::cmp::Ordering::Equal => {
                // First sub-tiebreaker: nanosecond precision (if available)
                let ns_a = a
                    .get("_timestamp_nanos")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let ns_b = b
                    .get("_timestamp_nanos")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                match ns_b.cmp(&ns_a) {
                    std::cmp::Ordering::Equal => {
                        // Second sub-tiebreaker: _signal (logs vs traces)
                        let sig_a = a.get("_signal").and_then(|v| v.as_str()).unwrap_or("");
                        let sig_b = b.get("_signal").and_then(|v| v.as_str()).unwrap_or("");
                        match sig_a.cmp(sig_b) {
                            std::cmp::Ordering::Equal => {
                                // Third sub-tiebreaker: id
                                let id_a = a.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let id_b = b.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                id_b.cmp(&id_a)
                            }
                            other => other,
                        }
                    }
                    other => other,
                }
            }
            other => other,
        }
    });
}

/// Create error response from ExportError.
fn error_response(err: &ExportError) -> Result<Response> {
    Response::error(&err.message, err.status_code)
}
