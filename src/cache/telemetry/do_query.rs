//! DO-level query handler for logs and traces.

#[cfg(target_arch = "wasm32")]
use worker::*;

#[cfg(target_arch = "wasm32")]
use crate::cache::types::DOQueryRequest;

/// Handle GET/POST /query - query records with filters.
#[cfg(target_arch = "wasm32")]
pub async fn handle_do_query(state: &State, mut req: Request) -> Result<Response> {
    let query_params = if req.method() == worker::Method::Post {
        let body = req.text().await?;
        serde_json::from_str::<DOQueryRequest>(&body)
            .map_err(|e| worker::Error::RustError(format!("Invalid query JSON: {}", e)))?
    } else {
        // Parse URL query params
        let url = req.url()?;
        let table = url
            .query_pairs()
            .find(|(k, _)| k == "table")
            .map(|(_, v)| v.to_string())
            .unwrap_or_else(|| "logs".to_string());
        let start_time = url
            .query_pairs()
            .find(|(k, _)| k == "start_time")
            .and_then(|(_, v)| v.parse::<i64>().ok());
        let end_time = url
            .query_pairs()
            .find(|(k, _)| k == "end_time")
            .and_then(|(_, v)| v.parse::<i64>().ok());
        let trace_id = url
            .query_pairs()
            .find(|(k, _)| k == "trace_id")
            .map(|(_, v)| v.to_string());
        let metric_name = url
            .query_pairs()
            .find(|(k, _)| k == "metric_name")
            .map(|(_, v)| v.to_string());
        let labels = url
            .query_pairs()
            .find(|(k, _)| k == "labels")
            .map(|(_, v)| parse_labels_query(&v))
            .unwrap_or_default();
        let limit = url
            .query_pairs()
            .find(|(k, _)| k == "limit")
            .and_then(|(_, v)| v.parse::<i64>().ok())
            .unwrap_or(100);

        DOQueryRequest {
            table,
            start_time,
            end_time,
            trace_id,
            metric_name,
            labels,
            limit,
        }
    };

    let results = query_records(state, &query_params)?;
    Response::from_json(&results)
}

/// Query records from SQLite based on filters.
#[cfg(target_arch = "wasm32")]
fn query_records(state: &State, query: &DOQueryRequest) -> Result<Vec<serde_json::Value>> {
    // Validate table name to prevent SQL injection
    if query.table != "logs"
        && query.table != "traces"
        && query.table != "gauge"
        && query.table != "sum"
    {
        return Err(worker::Error::RustError("Invalid table name".to_string()));
    }

    let sql_storage = state.storage().sql();
    let mut sql = format!("SELECT * FROM {}", query.table);
    let mut conditions = Vec::new();
    let mut bindings = Vec::new();

    if let Some(start) = query.start_time {
        conditions.push("timestamp >= ?".to_string());
        bindings.push(SqlStorageValue::Integer(start));
    }

    if let Some(end) = query.end_time {
        conditions.push("timestamp <= ?".to_string());
        bindings.push(SqlStorageValue::Integer(end));
    }

    if let Some(ref trace_id) = query.trace_id {
        conditions.push("trace_id = ?".to_string());
        bindings.push(SqlStorageValue::String(trace_id.clone()));
    }

    if (query.table == "gauge" || query.table == "sum") && query.metric_name.is_some() {
        if let Some(ref metric_name) = query.metric_name {
            conditions.push("metric_name = ?".to_string());
            bindings.push(SqlStorageValue::String(metric_name.clone()));
        }
    }

    if (query.table == "gauge" || query.table == "sum") && !query.labels.is_empty() {
        for (label_key, label_value) in &query.labels {
            let pattern = format!("%\"{}\":\"{}\"%", label_key, label_value);
            conditions.push("metric_attributes LIKE ?".to_string());
            bindings.push(SqlStorageValue::String(pattern));
        }
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    // Clamp limit to prevent negative or extremely large values
    let limit = query.limit.clamp(1, 10_000);
    sql.push_str(&format!(
        " ORDER BY timestamp DESC, id DESC LIMIT {}",
        limit
    ));

    let cursor = sql_storage.exec(&sql, bindings)?;

    // Convert cursor to JSON array
    let results: Vec<serde_json::Value> = cursor
        .to_array()
        .map_err(|e| worker::Error::RustError(format!("Failed to parse results: {}", e)))?;

    Ok(results)
}

#[cfg(target_arch = "wasm32")]
fn parse_labels_query(input: &str) -> Vec<(String, String)> {
    input
        .split(',')
        .filter_map(|label_pair| {
            let mut parts = label_pair.splitn(2, '=');
            let label_key = parts.next().unwrap_or("").trim();
            let label_value = parts.next().unwrap_or("").trim();
            if label_key.is_empty() {
                None
            } else {
                Some((label_key.to_string(), label_value.to_string()))
            }
        })
        .collect()
}
