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
use crate::stats::{handle_all_services_stats, handle_stats_query};
use crate::transform::init_programs;

/// Add CORS headers to a response.
/// Creates a new response to handle immutable headers from Durable Objects.
fn with_cors(response: Response) -> Result<Response> {
    let headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, DELETE, OPTIONS",
    )?;
    headers.set(
        "Access-Control-Allow-Headers",
        "Content-Type, Accept, Content-Encoding, Authorization, X-User-Agent, X-Iceberg-Access-Delegation",
    )?;
    headers.set("Access-Control-Max-Age", "86400")?;

    // Copy original headers
    for (key, value) in response.headers() {
        // Don't overwrite CORS headers
        if !key.to_lowercase().starts_with("access-control-") {
            headers.set(&key, &value)?;
        }
    }

    Ok(Response::from_body(response.body().clone())?
        .with_status(response.status_code())
        .with_headers(headers))
}

/// Handle CORS preflight OPTIONS requests.
fn cors_preflight() -> Result<Response> {
    with_cors(Response::empty()?.with_status(204))
}

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

    // Handle CORS preflight for all API endpoints
    if method == Method::Options {
        return cors_preflight();
    }

    let response = match (method, path.as_str()) {
        (Method::Post, "/v1/logs") => handle_logs_worker(req, env, ctx).await,
        (Method::Post, "/v1/traces") => handle_traces_worker(req, env, ctx).await,
        (Method::Post, "/v1/metrics") => handle_metrics_worker(req, env, ctx).await,
        (Method::Post, "/services/collector/event") => handle_hec_logs_worker(req, env, ctx).await,
        (Method::Get, "/health") => Response::ok("ok"),
        (Method::Get, "/v1/config") => handle_config(env),
        (Method::Get, "/v1/services") => handle_services_list(env).await,
        // All-services stats: /v1/services/stats?signal=logs|traces
        (Method::Get, "/v1/services/stats") => handle_all_services_stats(req, env).await,
        // Per-service stats: /v1/services/:service/:signal/stats
        (Method::Get, path) if path.starts_with("/v1/services/") && path.ends_with("/stats") => {
            handle_stats_query(path, req, env).await
        }
        // Live tail WebSocket upgrade - return directly without CORS wrapper
        // WebSocket responses use status 101 which can't be reconstructed
        (Method::Get, path) if path.starts_with("/v1/tail/") => {
            return handle_tail_upgrade(path, req, env).await;
        }
        // R2 Data Catalog proxy for browser DuckDB (CORS workaround)
        (_, path) if path.starts_with("/v1/iceberg/") => handle_iceberg_proxy(path, req, env).await,
        _ => Response::error("Not Found", 404),
    };

    // Add CORS headers to all responses, including errors
    match response {
        Ok(r) => with_cors(r),
        Err(e) => with_cors(Response::error(e.to_string(), 500)?),
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

/// Return R2 catalog configuration for frontend DuckDB connection.
/// The token is NOT returned - it's injected by the /v1/iceberg proxy.
fn handle_config(env: Env) -> Result<Response> {
    let account_id = env.var("R2_CATALOG_ACCOUNT_ID").map(|v| v.to_string()).ok();
    let bucket_name = env.var("R2_CATALOG_BUCKET").map(|v| v.to_string()).ok();
    let token_configured = env.secret("R2_CATALOG_TOKEN").is_ok();
    let mut missing = Vec::new();
    if account_id.is_none() {
        missing.push("R2_CATALOG_ACCOUNT_ID");
    }
    if bucket_name.is_none() {
        missing.push("R2_CATALOG_BUCKET");
    }
    if !token_configured {
        missing.push("R2_CATALOG_TOKEN");
    }
    if !missing.is_empty() {
        tracing::warn!(
            missing = ?missing,
            "Iceberg catalog proxy disabled: missing configuration"
        );
    }

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ConfigResponse {
        account_id: Option<String>,
        bucket_name: Option<String>,
        iceberg_proxy_enabled: bool,
    }

    let config = ConfigResponse {
        iceberg_proxy_enabled: missing.is_empty(),
        account_id,
        bucket_name,
    };

    Response::from_json(&config)
}

/// Proxy requests to R2 Data Catalog to work around browser CORS restrictions.
/// The worker adds auth headers and forwards to catalog.cloudflarestorage.com.
///
/// Path format: /v1/iceberg/{rest_of_path}
/// Environment variables required:
///   - R2_CATALOG_ACCOUNT_ID: Cloudflare account ID
///   - R2_CATALOG_BUCKET: R2 bucket name
///   - R2_CATALOG_TOKEN: R2 API token (secret)
async fn handle_iceberg_proxy(path: &str, mut req: Request, env: Env) -> Result<Response> {
    // Get configuration from environment
    let account_id = env
        .var("R2_CATALOG_ACCOUNT_ID")
        .map(|v| v.to_string())
        .map_err(|_| Error::from("R2_CATALOG_ACCOUNT_ID not configured"))?;
    let bucket = env
        .var("R2_CATALOG_BUCKET")
        .map(|v| v.to_string())
        .map_err(|_| Error::from("R2_CATALOG_BUCKET not configured"))?;
    let token = env
        .secret("R2_CATALOG_TOKEN")
        .map(|v| v.to_string())
        .map_err(|_| Error::from("R2_CATALOG_TOKEN secret not configured"))?;

    // Extract the path after /v1/iceberg/
    let catalog_path = path.trim_start_matches("/v1/iceberg");

    // Build the target URL
    let catalog_base = format!(
        "https://catalog.cloudflarestorage.com/{}/{}",
        account_id, bucket
    );
    let target_url = if catalog_path.is_empty() || catalog_path == "/" {
        catalog_base
    } else {
        format!("{}{}", catalog_base, catalog_path)
    };

    // Preserve query string if present
    let url = req.url()?;
    let target_url = if let Some(query) = url.query() {
        format!("{}?{}", target_url, query)
    } else {
        target_url
    };

    // Build headers for the proxied request
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", token))?;

    // Copy relevant headers from original request
    if let Ok(Some(content_type)) = req.headers().get("Content-Type") {
        headers.set("Content-Type", &content_type)?;
    }
    if let Ok(Some(accept)) = req.headers().get("Accept") {
        headers.set("Accept", &accept)?;
    }

    // Get method and body before creating request
    let method = req.method();
    let is_body_request = method == Method::Post || method == Method::Put;
    let body = if is_body_request {
        Some(req.bytes().await?)
    } else {
        None
    };

    // Create the proxied request
    let mut init = RequestInit::new();
    init.with_method(method);
    init.with_headers(headers);
    if let Some(b) = body {
        init.with_body(Some(b.into()));
    }

    let proxy_req = Request::new_with_init(&target_url, &init)?;

    // Execute the request
    let response = Fetch::Request(proxy_req).send().await?;

    // Log non-2xx responses for debugging catalog issues
    if response.status_code() >= 400 {
        tracing::warn!(
            status = response.status_code(),
            path = catalog_path,
            "R2 catalog proxy received error response"
        );
    }

    Ok(response)
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

    // Build headers for WebSocket upgrade
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

    let do_request = worker::Request::new_with_init(
        "http://do/websocket",
        worker::RequestInit::new()
            .with_method(worker::Method::Get)
            .with_headers(headers),
    )?;

    // Forward to Durable Object and return response directly
    // Note: WebSocket responses (status 101) cannot be modified or wrapped with CORS
    stub.fetch_with_request(do_request).await
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
