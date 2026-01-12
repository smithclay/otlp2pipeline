use axum::{
    body::Bytes as AxumBytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::handler::{
    handle_signal, HandleResponse, LogsHandler, MetricsHandler, SignalHandler, TracesHandler,
};
use crate::parse_content_metadata;
use crate::pipeline::PipelineClient;
use crate::signal::Signal;
use crate::Bytes;
use crate::InputFormat;

/// Initialize tracing subscriber for native (non-WASM) builds.
/// Uses RUST_LOG env var for filtering (defaults to info).
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(true))
        .with(filter)
        .init();
}

pub fn build_router(pipeline_url: String) -> Router {
    let mut endpoints = HashMap::new();
    endpoints.insert(Signal::Logs, pipeline_url.clone());
    endpoints.insert(Signal::Traces, pipeline_url.clone());
    endpoints.insert(Signal::Gauge, pipeline_url.clone());
    endpoints.insert(Signal::Sum, pipeline_url);

    let client = Arc::new(PipelineClient::new(endpoints, "test-token".to_string()));
    build_router_with_client(client)
}

pub fn build_router_multi(endpoints: std::collections::HashMap<Signal, String>) -> Router {
    let client = Arc::new(PipelineClient::new(endpoints, "test-token".to_string()));
    build_router_with_client(client)
}

fn build_router_with_client(client: Arc<PipelineClient>) -> Router {
    Router::new()
        .route("/v1/logs", post(handle_logs_axum))
        .route("/v1/traces", post(handle_traces_axum))
        .route("/v1/metrics", post(handle_metrics_axum))
        .route("/health", get(|| async { "ok" }))
        .with_state(client)
}

async fn handle_axum_signal<H: SignalHandler>(
    headers: HeaderMap,
    body: AxumBytes,
    client: &PipelineClient,
) -> Result<Json<HandleResponse>, (StatusCode, String)> {
    let (is_gzipped, decode_format) = parse_axum_headers(&headers);

    handle_signal::<H, _>(
        Bytes::from(body.to_vec()),
        is_gzipped,
        decode_format,
        client,
    )
    .await
    .map(Json)
    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn handle_logs_axum(
    State(client): State<Arc<PipelineClient>>,
    headers: HeaderMap,
    body: AxumBytes,
) -> Result<Json<HandleResponse>, (StatusCode, String)> {
    handle_axum_signal::<LogsHandler>(headers, body, client.as_ref()).await
}

async fn handle_traces_axum(
    State(client): State<Arc<PipelineClient>>,
    headers: HeaderMap,
    body: AxumBytes,
) -> Result<Json<HandleResponse>, (StatusCode, String)> {
    handle_axum_signal::<TracesHandler>(headers, body, client.as_ref()).await
}

async fn handle_metrics_axum(
    State(client): State<Arc<PipelineClient>>,
    headers: HeaderMap,
    body: AxumBytes,
) -> Result<Json<HandleResponse>, (StatusCode, String)> {
    handle_axum_signal::<MetricsHandler>(headers, body, client.as_ref()).await
}

fn parse_axum_headers(headers: &HeaderMap) -> (bool, InputFormat) {
    parse_content_metadata(|name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    })
}
