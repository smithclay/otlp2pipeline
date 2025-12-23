use bytes::Bytes;
use time::format_description::well_known::Rfc3339;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};
use worker::*;

use crate::decode::DecodeFormat;
use crate::handler;
use crate::parse_content_metadata;
use crate::pipeline::PipelineClient;
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
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let method = req.method();
    let path = req.path();

    match (method, path.as_str()) {
        (Method::Post, "/v1/logs") => handle_logs_worker(req, env).await,
        (Method::Post, "/v1/traces") => handle_traces_worker(req, env).await,
        (Method::Post, "/v1/metrics") => handle_metrics_worker(req, env).await,
        (Method::Post, "/services/collector/event") => handle_hec_logs_worker(req, env).await,
        (Method::Get, "/health") => Response::ok("ok"),
        _ => Response::error("Not Found", 404),
    }
}

async fn handle_signal_worker<H: handler::SignalHandler>(
    mut req: Request,
    env: Env,
    override_format: Option<DecodeFormat>,
) -> Result<Response> {
    let body_bytes = req.bytes().await?;
    let (is_gzipped, decode_format) = parse_worker_headers(&req);
    let decode_format = override_format.unwrap_or(decode_format);
    let client = PipelineClient::from_worker_env(&env)?;

    match handler::handle_signal::<H, _>(
        Bytes::from(body_bytes),
        is_gzipped,
        decode_format,
        &client,
    )
    .await
    {
        Ok(resp) => Response::from_json(&resp),
        Err(e) => Response::error(e.to_string(), 400),
    }
}

async fn handle_metrics_worker(req: Request, env: Env) -> Result<Response> {
    handle_signal_worker::<handler::MetricsHandler>(req, env, None).await
}

async fn handle_logs_worker(req: Request, env: Env) -> Result<Response> {
    handle_signal_worker::<handler::LogsHandler>(req, env, None).await
}

async fn handle_traces_worker(req: Request, env: Env) -> Result<Response> {
    handle_signal_worker::<handler::TracesHandler>(req, env, None).await
}

async fn handle_hec_logs_worker(req: Request, env: Env) -> Result<Response> {
    // HEC is always JSON, ignore content-type
    handle_signal_worker::<handler::HecLogsHandler>(req, env, Some(DecodeFormat::Json)).await
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
