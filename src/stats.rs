//! Stats API handlers for querying aggregated telemetry data.

#[cfg(target_arch = "wasm32")]
use crate::registry::RegistrySender;
#[cfg(target_arch = "wasm32")]
use crate::registry::WasmRegistrySender;
#[cfg(target_arch = "wasm32")]
use worker::*;

/// Stats row from an AggregatorDO - matches the StatsRow type in aggregator/durable_object.rs
#[cfg(target_arch = "wasm32")]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StatsRow {
    minute: i64,
    count: i64,
    error_count: i64,
    #[serde(default)]
    latency_sum_us: i64,
    #[serde(default)]
    latency_min_us: Option<i64>,
    #[serde(default)]
    latency_max_us: Option<i64>,
}

/// Per-service stats response for the all-services stats endpoint.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, serde::Serialize)]
struct ServiceStats {
    service: String,
    stats: Vec<StatsRow>,
}

/// Get stats for all services.
/// GET /v1/services/stats?signal=logs|traces&from=X&to=Y
#[cfg(target_arch = "wasm32")]
pub async fn handle_all_services_stats(req: Request, env: Env) -> Result<Response> {
    let url = req.url()?;
    let params: std::collections::HashMap<_, _> = url.query_pairs().collect();

    // Validate signal param (required)
    let signal = match params.get("signal").map(|s| s.as_ref()) {
        Some("logs") => "logs",
        Some("traces") => "traces",
        Some(_) => return Response::error("Signal must be 'logs' or 'traces'", 400),
        None => return Response::error("Missing required 'signal' query parameter", 400),
    };

    // Get all services from registry
    let sender = WasmRegistrySender::new(env.clone());
    let all_services = match sender.get_all_services().await {
        Ok(services) => services,
        Err(e) => return Response::error(format!("Failed to get services: {}", e), 500),
    };

    // Filter to services with the requested signal
    let services_with_signal: Vec<_> = all_services
        .into_iter()
        .filter(|s| {
            if signal == "logs" {
                s.has_logs > 0
            } else {
                s.has_traces > 0
            }
        })
        .collect();

    // Build query string for DO requests (preserve from/to params)
    let existing_query = url.query().unwrap_or("");
    let sep = if existing_query.is_empty() { "" } else { "&" };
    let do_query = format!("{}{}signal={}", existing_query, sep, signal);

    // Fan out to all service AggregatorDOs in parallel
    let namespace = env.durable_object("AGGREGATOR")?;
    let mut futures = Vec::with_capacity(services_with_signal.len());

    for service in &services_with_signal {
        let do_name = format!("{}:{}", service.name, signal);
        let id = namespace.id_from_name(&do_name)?;
        let stub = id.get_stub()?;
        let do_url = format!("http://do/stats?{}", do_query);
        let request = worker::Request::new(&do_url, worker::Method::Get)?;
        let service_name = service.name.clone();

        futures.push(async move {
            let result = stub.fetch_with_request(request).await;
            (service_name, result)
        });
    }

    // Await all futures concurrently
    let results = futures::future::join_all(futures).await;

    // Collect results
    let mut service_stats: Vec<ServiceStats> = Vec::with_capacity(results.len());
    for (service_name, result) in results {
        match result {
            Ok(mut response) if response.status_code() < 400 => {
                if let Ok(text) = response.text().await {
                    if let Ok(stats) = serde_json::from_str::<Vec<StatsRow>>(&text) {
                        service_stats.push(ServiceStats {
                            service: service_name,
                            stats,
                        });
                    } else {
                        // Log parse error but include service with empty stats
                        tracing::warn!(service = %service_name, "Failed to parse stats response");
                        service_stats.push(ServiceStats {
                            service: service_name,
                            stats: vec![],
                        });
                    }
                }
            }
            Ok(response) => {
                // Log error but include service with empty stats
                tracing::warn!(
                    service = %service_name,
                    status = response.status_code(),
                    "AggregatorDO returned error"
                );
                service_stats.push(ServiceStats {
                    service: service_name,
                    stats: vec![],
                });
            }
            Err(e) => {
                // Log error but include service with empty stats
                tracing::warn!(service = %service_name, error = %e, "Failed to fetch from AggregatorDO");
                service_stats.push(ServiceStats {
                    service: service_name,
                    stats: vec![],
                });
            }
        }
    }

    // Sort by service name for consistent ordering
    service_stats.sort_by(|a, b| a.service.cmp(&b.service));

    Response::from_json(&service_stats)
}

/// Get stats for a single service.
/// GET /v1/services/:service/:signal/stats?from=X&to=Y
#[cfg(target_arch = "wasm32")]
pub async fn handle_stats_query(path: &str, req: Request, env: Env) -> Result<Response> {
    // Parse path: /v1/services/:service/:signal/stats
    let parts: Vec<&str> = path
        .trim_start_matches("/v1/services/")
        .split('/')
        .collect();
    if parts.len() < 3 || parts[2] != "stats" {
        return Response::error("Invalid path. Use /v1/services/:service/:signal/stats", 400);
    }

    let service = parts[0];
    let signal = parts[1];

    // Validate signal
    if signal != "logs" && signal != "traces" {
        return Response::error("Signal must be 'logs' or 'traces'", 400);
    }

    let do_name = format!("{}:{}", service, signal);

    let namespace = env.durable_object("AGGREGATOR")?;
    let id = namespace.id_from_name(&do_name)?;
    let stub = id.get_stub()?;

    // Forward request to DO (preserving query string and adding signal param)
    let url = req.url()?;
    let existing_query = url.query().unwrap_or("");
    let sep = if existing_query.is_empty() { "" } else { "&" };
    let do_url = format!("http://do/stats?{}{}signal={}", existing_query, sep, signal);

    let request = worker::Request::new(&do_url, worker::Method::Get)?;
    stub.fetch_with_request(request).await
}
