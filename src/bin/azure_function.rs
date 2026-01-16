//! Azure Functions entry point for OTLP ingestion.
//!
//! Build with: docker build -f Dockerfile.azure -t otlp2pipeline-azure .
//! Local dev:  cargo run --features azure-function --bin azure_function

use axum::{
    body::Bytes,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use otlp2pipeline::{
    azure::{EventHubConfig, EventHubSender},
    handle_signal, HandleError, InputFormat, LogsHandler, MetricsHandler, SignalHandler,
    TracesHandler,
};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Optional auth token loaded at cold start
static AUTH_TOKEN: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

fn init_auth_token() {
    let token =
        AUTH_TOKEN.get_or_init(|| std::env::var("AUTH_TOKEN").ok().filter(|t| !t.is_empty()));
    if token.is_some() {
        info!("AUTH_TOKEN configured - authentication enabled");
    } else {
        warn!("AUTH_TOKEN not set - authentication disabled");
    }
}

/// Constant-time comparison for auth tokens
#[inline]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn check_auth(headers: &HeaderMap) -> Result<(), (StatusCode, &'static str)> {
    let expected = match AUTH_TOKEN.get().and_then(|t| t.as_ref()) {
        Some(token) => token,
        None => return Ok(()),
    };

    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    match provided {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => Ok(()),
        Some(_) => Err((StatusCode::UNAUTHORIZED, "Invalid token")),
        None => Err((StatusCode::UNAUTHORIZED, "Missing Authorization header")),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .json()
        .init();

    info!("Azure Function cold start - initializing");

    init_auth_token();

    let config = EventHubConfig::from_env().map_err(|e| {
        error!(error = %e, "Failed to load Event Hub config");
        e
    })?;

    let sender = Arc::new(EventHubSender::new(config).await.map_err(|e| {
        error!(error = %e, "Failed to create Event Hub sender");
        e
    })?);

    let app = Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .route("/v1/logs", post(handle_logs))
        .route("/v1/traces", post(handle_traces))
        .route("/v1/metrics", post(handle_metrics))
        .with_state(sender);

    // Azure Functions custom handler listens on FUNCTIONS_CUSTOMHANDLER_PORT
    let port = std::env::var("FUNCTIONS_CUSTOMHANDLER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!(port = port, "Listening");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "OK"
}

async fn handle_logs(
    headers: HeaderMap,
    state: axum::extract::State<Arc<EventHubSender>>,
    body: Bytes,
) -> impl IntoResponse {
    handle_signal_request::<LogsHandler>(headers, &state, body).await
}

async fn handle_traces(
    headers: HeaderMap,
    state: axum::extract::State<Arc<EventHubSender>>,
    body: Bytes,
) -> impl IntoResponse {
    handle_signal_request::<TracesHandler>(headers, &state, body).await
}

async fn handle_metrics(
    headers: HeaderMap,
    state: axum::extract::State<Arc<EventHubSender>>,
    body: Bytes,
) -> impl IntoResponse {
    handle_signal_request::<MetricsHandler>(headers, &state, body).await
}

async fn handle_signal_request<H: SignalHandler>(
    headers: HeaderMap,
    sender: &EventHubSender,
    body: Bytes,
) -> impl IntoResponse {
    if let Err((status, msg)) = check_auth(&headers) {
        return (status, msg.to_string());
    }

    let is_gzipped = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("gzip"))
        .unwrap_or(false);

    let format = InputFormat::from_content_type(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
    );

    match handle_signal::<H, _>(body, is_gzipped, format, sender).await {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(json) => (StatusCode::OK, json),
            Err(e) => {
                error!(error = %e, "Failed to serialize response");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Serialization error".to_string(),
                )
            }
        },
        Err(e) => {
            let (status, msg) = match &e {
                HandleError::Decompress(m) => {
                    (StatusCode::BAD_REQUEST, format!("Decompress: {}", m))
                }
                HandleError::Decode(m) => (StatusCode::BAD_REQUEST, format!("Decode: {}", m)),
                HandleError::Transform(m) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Transform: {}", m),
                ),
                HandleError::SendFailed(m) => (StatusCode::BAD_GATEWAY, format!("Send: {}", m)),
            };
            (status, msg)
        }
    }
}
