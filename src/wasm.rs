use bytes::Bytes;
use time::format_description::well_known::Rfc3339;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};
use worker::*;

use crate::decode::DecodeFormat;
use crate::handler;
use crate::livetail::WasmLiveTailSender;
use crate::parse_content_metadata;
use crate::pipeline::PipelineClient;
use crate::registry::{RegistrySender, WasmRegistrySender};
use crate::signal::Signal;
use crate::transform::init_programs;

/// Initialize tracing and VRL programs for Cloudflare Workers.
/// Must be called via #[event(start)] to run once on worker initialization.
#[event(start)]
fn init() {
    // JSON formatting layer that writes to the Workers console
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_ansi(false)
        .with_timer(UtcTime::new(Rfc3339))
        .with_writer(MakeWebConsoleWriter::new());

    let perf_layer = performance_layer().with_details_from_fields(Pretty::default());

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(perf_layer)
        .init();

    // Pre-compile VRL programs to avoid cold-start latency
    init_programs();
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let method = req.method();
    let path = req.path();

    match (method, path.as_str()) {
        (Method::Post, "/v1/logs") => handle_logs_worker(req, env, ctx).await,
        (Method::Post, "/v1/traces") => handle_traces_worker(req, env, ctx).await,
        (Method::Post, "/v1/metrics") => handle_metrics_worker(req, env, ctx).await,
        (Method::Post, "/services/collector/event") => handle_hec_logs_worker(req, env, ctx).await,
        (Method::Get, "/health") => Response::ok("ok"),
        (Method::Get, "/v1/services") => handle_services_list(env).await,
        // Stats API endpoints
        (Method::Get, path) if path.starts_with("/v1/services/") => {
            handle_stats_query(path, req, env).await
        }
        // Live tail WebSocket upgrade
        (Method::Get, path) if path.starts_with("/v1/tail/") => {
            handle_tail_upgrade(path, req, env).await
        }
        _ => Response::error("Not Found", 404),
    }
}

async fn handle_signal_worker<H: handler::SignalHandler>(
    mut req: Request,
    env: Env,
    ctx: Context,
    override_format: Option<DecodeFormat>,
) -> Result<Response> {
    let body_bytes = req.bytes().await?;
    let (is_gzipped, decode_format) = parse_worker_headers(&req);
    let decode_format = override_format.unwrap_or(decode_format);
    let client = PipelineClient::from_worker_env(&env)?;

    // Initialize aggregator sender for dual-write
    let cache = crate::aggregator::WasmAggregatorSender::new(env.clone());

    // Initialize livetail sender for triple-write
    let livetail = WasmLiveTailSender::new(env.clone());

    match handler::handle_signal_with_cache::<H, _, _, _>(
        Bytes::from(body_bytes),
        is_gzipped,
        decode_format,
        &client,
        Some(&cache),
        Some(&livetail),
    )
    .await
    {
        Ok(resp) => {
            // Fire-and-forget service registration for discovered services
            if !resp.service_names.is_empty() {
                let env_clone = env.clone();
                let service_names = resp.service_names.clone();
                let signal = H::SIGNAL;
                ctx.wait_until(async move {
                    register_services(&env_clone, &service_names, signal).await;
                });
            }
            Response::from_json(&resp)
        }
        Err(e) => Response::error(e.to_string(), 400),
    }
}

async fn handle_metrics_worker(req: Request, env: Env, ctx: Context) -> Result<Response> {
    handle_signal_worker::<handler::MetricsHandler>(req, env, ctx, None).await
}

async fn handle_logs_worker(req: Request, env: Env, ctx: Context) -> Result<Response> {
    handle_signal_worker::<handler::LogsHandler>(req, env, ctx, None).await
}

async fn handle_traces_worker(req: Request, env: Env, ctx: Context) -> Result<Response> {
    handle_signal_worker::<handler::TracesHandler>(req, env, ctx, None).await
}

async fn handle_hec_logs_worker(req: Request, env: Env, ctx: Context) -> Result<Response> {
    // HEC is always JSON, ignore content-type
    handle_signal_worker::<handler::HecLogsHandler>(req, env, ctx, Some(DecodeFormat::Json)).await
}

fn parse_worker_headers(req: &Request) -> (bool, DecodeFormat) {
    parse_content_metadata(|name| {
        req.headers()
            .get(name)
            .ok()
            .flatten()
            .map(|s| s.to_string())
    })
}

/// Register services with RegistryDO (fire-and-forget helper).
async fn register_services(env: &Env, service_names: &[String], signal: Signal) {
    if service_names.is_empty() {
        return;
    }

    let sender = WasmRegistrySender::new(env.clone());

    if let Err(e) = sender
        .register_services(service_names.to_vec(), signal)
        .await
    {
        tracing::warn!(error = %e, signal = ?signal, "Failed to register services");
    }
}

async fn handle_services_list(env: Env) -> Result<Response> {
    let sender = WasmRegistrySender::new(env);

    match sender.get_all_services().await {
        Ok(services) => Response::from_json(&services),
        Err(e) => Response::error(format!("Failed to get services: {}", e), 500),
    }
}

async fn handle_stats_query(path: &str, req: Request, env: Env) -> Result<Response> {
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

    // Forward request to DO (preserving query string)
    let url = req.url()?;
    let query = url.query().map(|q| format!("?{}", q)).unwrap_or_default();
    let do_url = format!("http://do/stats{}", query);

    let request = worker::Request::new(&do_url, worker::Method::Get)?;
    stub.fetch_with_request(request).await
}

async fn handle_tail_upgrade(path: &str, req: Request, env: Env) -> Result<Response> {
    // Parse path: /v1/tail/:service/:signal
    let parts: Vec<&str> = path.trim_start_matches("/v1/tail/").split('/').collect();

    if parts.len() < 2 {
        return Response::error("Invalid path. Use /v1/tail/:service/:signal", 400);
    }

    let service = parts[0];
    let signal = parts[1];

    // Validate signal
    if signal != "logs" && signal != "traces" {
        return Response::error("Signal must be 'logs' or 'traces'", 400);
    }

    // Validate service name (same rules as aggregator)
    if service.is_empty()
        || service.len() > 128
        || !service
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Response::error("Invalid service name", 400);
    }

    let do_name = format!("{}:{}", service, signal);

    let namespace = env.durable_object("LIVETAIL")?;
    let id = namespace.id_from_name(&do_name)?;
    let stub = id.get_stub()?;

    // Forward WebSocket upgrade to DO
    let headers = worker::Headers::new();
    if let Ok(Some(upgrade)) = req.headers().get("Upgrade") {
        headers.set("Upgrade", &upgrade)?;
    }
    if let Ok(Some(key)) = req.headers().get("Sec-WebSocket-Key") {
        headers.set("Sec-WebSocket-Key", &key)?;
    }
    if let Ok(Some(version)) = req.headers().get("Sec-WebSocket-Version") {
        headers.set("Sec-WebSocket-Version", &version)?;
    }

    let request = worker::Request::new_with_init(
        "http://do/websocket",
        worker::RequestInit::new()
            .with_method(worker::Method::Get)
            .with_headers(headers),
    )?;

    stub.fetch_with_request(request).await
}

// Re-export AggregatorDO from aggregator module
#[allow(unused_imports)]
pub use crate::aggregator::AggregatorDO;

// Re-export RegistryDO from registry module
#[allow(unused_imports)]
pub use crate::registry::RegistryDO;

// Re-export LiveTailDO from livetail module
#[allow(unused_imports)]
pub use crate::livetail::LiveTailDO;
