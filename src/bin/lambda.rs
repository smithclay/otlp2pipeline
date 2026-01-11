//! AWS Lambda entry point for OTLP ingestion.
//!
//! Build with: cargo lambda build --release --arm64 --features lambda --bin lambda
//! Deploy artifact: target/lambda/lambda/bootstrap

use lambda_http::{run, service_fn, Body, Error, Request, Response};
use otlp2pipeline::{
    handle_signal,
    lambda::firehose::{FirehoseSender, StreamConfig},
    DecodeFormat, HandleError, HecLogsHandler, LogsHandler, MetricsHandler, TracesHandler,
};
use std::sync::Arc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing for CloudWatch Logs
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .json()
        .with_target(false)
        .without_time() // Lambda adds timestamps
        .init();

    info!("Lambda cold start - initializing Firehose client");

    // Load stream configuration from environment
    let streams = StreamConfig::from_env().map_err(Error::from)?;

    // Create Firehose sender (reused across invocations)
    let sender = Arc::new(FirehoseSender::new(streams).await);

    run(service_fn(|event| handler(event, sender.clone()))).await
}

async fn handler(event: Request, sender: Arc<FirehoseSender>) -> Result<Response<Body>, Error> {
    let path = event.uri().path().to_string();
    let method = event.method().clone();

    // Health check endpoint
    if path == "/health" || path == "/" {
        return Ok(Response::builder()
            .status(200)
            .body(Body::from("OK"))
            .unwrap());
    }

    // Only accept POST for telemetry endpoints
    if method != "POST" {
        return Ok(Response::builder()
            .status(405)
            .body(Body::from("Method not allowed"))
            .unwrap());
    }

    // Parse content metadata from headers
    let is_gzipped = event
        .headers()
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("gzip"))
        .unwrap_or(false);

    let format = DecodeFormat::from_content_type(
        event
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
    );

    // Get body as bytes
    // Body is non-exhaustive, so we must handle unknown variants
    let body = event.into_body();
    let body_bytes = match body {
        Body::Empty => bytes::Bytes::new(),
        Body::Text(s) => bytes::Bytes::from(s),
        Body::Binary(b) => bytes::Bytes::from(b),
        other => {
            // Non-exhaustive enum: future variants should be handled explicitly
            // Log and reject rather than silently dropping data
            error!(body_type = ?other, path = %path, "unsupported body type");
            return Ok(Response::builder()
                .status(400)
                .body(Body::from("Unsupported body type"))
                .unwrap());
        }
    };

    // Route to appropriate handler
    let result = match path.as_str() {
        "/v1/logs" => {
            handle_signal::<LogsHandler, _>(body_bytes, is_gzipped, format, &*sender).await
        }
        "/v1/traces" => {
            handle_signal::<TracesHandler, _>(body_bytes, is_gzipped, format, &*sender).await
        }
        "/v1/metrics" => {
            handle_signal::<MetricsHandler, _>(body_bytes, is_gzipped, format, &*sender).await
        }
        "/services/collector" | "/services/collector/event" => {
            // Splunk HEC is always JSON, ignore content-type
            handle_signal::<HecLogsHandler, _>(body_bytes, is_gzipped, DecodeFormat::Json, &*sender)
                .await
        }
        _ => {
            return Ok(Response::builder()
                .status(404)
                .body(Body::from("Not found"))
                .unwrap());
        }
    };

    match result {
        Ok(response) => {
            let json = match serde_json::to_string(&response) {
                Ok(j) => j,
                Err(e) => {
                    error!(error = %e, "failed to serialize response");
                    r#"{"status":"ok","serialization_error":true}"#.to_string()
                }
            };
            Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from(json))
                .unwrap())
        }
        Err(e) => {
            let (status, message) = match &e {
                HandleError::Decompress(msg) => {
                    warn!(error = %msg, path = %path, "decompression error");
                    (400, format!("Decompression error: {}", msg))
                }
                HandleError::Decode(msg) => {
                    warn!(error = %msg, path = %path, "decode error");
                    (400, format!("Decode error: {}", msg))
                }
                HandleError::Transform(msg) => {
                    error!(error = %msg, path = %path, "transform error");
                    (500, format!("Transform error: {}", msg))
                }
                HandleError::SendFailed(msg) => {
                    error!(error = %msg, path = %path, "send failed");
                    (502, format!("Send failed: {}", msg))
                }
            };
            Ok(Response::builder()
                .status(status)
                .body(Body::from(message))
                .unwrap())
        }
    }
}
