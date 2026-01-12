//! AWS Lambda entry point for OTLP ingestion.
//!
//! Build with: cargo lambda build --release --arm64 --features lambda --bin lambda
//! Deploy artifact: target/lambda/lambda/bootstrap

use lambda_http::{run, service_fn, Body, Error, Request, Response};
use otlp2pipeline::{
    handle_signal,
    lambda::firehose::{FirehoseSender, StreamConfig},
    HandleError, InputFormat, LogsHandler, MetricsHandler, TracesHandler,
};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Constant-time byte comparison to prevent timing attacks on auth tokens.
/// Returns true if both slices are equal, using XOR accumulation to ensure
/// the comparison time is independent of where differences occur.
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

/// Optional auth token loaded at cold start
static AUTH_TOKEN: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Initialize auth token from environment (call once at cold start)
fn init_auth_token() {
    let token =
        AUTH_TOKEN.get_or_init(|| std::env::var("AUTH_TOKEN").ok().filter(|t| !t.is_empty()));
    if token.is_some() {
        info!("AUTH_TOKEN configured - authentication enabled");
    } else {
        warn!("AUTH_TOKEN not set - authentication disabled, endpoints are unprotected");
    }
}

/// Validate bearer token if AUTH_TOKEN env var is set.
/// Returns Ok(()) if auth is valid or not required, Err(Response) if unauthorized.
#[allow(clippy::result_large_err)] // Response<Body> is large but acceptable here
fn check_auth(req: &Request) -> Result<(), Response<Body>> {
    let expected_token = match AUTH_TOKEN.get().and_then(|t| t.as_ref()) {
        Some(token) => token,
        None => return Ok(()), // Auth disabled
    };

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let provided_token = match auth_header {
        Some(header) => match header.strip_prefix("Bearer ") {
            Some(token) => token,
            None => {
                return Err(Response::builder()
                    .status(401)
                    .body(Body::from(
                        "Unauthorized: invalid Authorization header format",
                    ))
                    .unwrap());
            }
        },
        None => {
            return Err(Response::builder()
                .status(401)
                .body(Body::from("Unauthorized: missing Authorization header"))
                .unwrap());
        }
    };

    // Use constant-time comparison to prevent timing attacks
    if !constant_time_eq(provided_token.as_bytes(), expected_token.as_bytes()) {
        return Err(Response::builder()
            .status(401)
            .body(Body::from("Unauthorized: invalid token"))
            .unwrap());
    }

    Ok(())
}

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

    info!("Lambda cold start - initializing");

    // Load auth token from environment (optional)
    init_auth_token();

    // Load stream configuration from environment
    let streams = StreamConfig::from_env().map_err(Error::from)?;

    // Create Firehose sender (reused across invocations)
    let sender = Arc::new(FirehoseSender::new(streams).await);

    run(service_fn(|event| handler(event, sender.clone()))).await
}

async fn handler(event: Request, sender: Arc<FirehoseSender>) -> Result<Response<Body>, Error> {
    let path = event.uri().path().to_string();
    let method = event.method().clone();

    // Health check endpoint (no auth required)
    if path == "/health" || path == "/" {
        return Ok(Response::builder()
            .status(200)
            .body(Body::from("OK"))
            .unwrap());
    }

    // Check auth if AUTH_TOKEN is configured
    if let Err(response) = check_auth(&event) {
        return Ok(response);
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

    let format = InputFormat::from_content_type(
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
        _ => {
            return Ok(Response::builder()
                .status(404)
                .body(Body::from("Not found"))
                .unwrap());
        }
    };

    match result {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(json) => Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(Body::from(json))
                .unwrap()),
            Err(e) => {
                error!(error = %e, "failed to serialize response");
                Ok(Response::builder()
                    .status(500)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"error":"Internal error: response serialization failed"}"#,
                    ))
                    .unwrap())
            }
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq_equal_strings() {
        assert!(constant_time_eq(b"test-token-123", b"test-token-123"));
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"a", b"a"));
    }

    #[test]
    fn test_constant_time_eq_different_strings() {
        assert!(!constant_time_eq(b"test-token-123", b"test-token-124"));
        assert!(!constant_time_eq(b"test-token-123", b"wrong-token"));
        assert!(!constant_time_eq(b"a", b"b"));
    }

    #[test]
    fn test_constant_time_eq_different_lengths() {
        assert!(!constant_time_eq(b"short", b"longer-string"));
        assert!(!constant_time_eq(b"", b"non-empty"));
        assert!(!constant_time_eq(b"test-token-123", b"test-token-12"));
    }

    #[test]
    fn test_constant_time_eq_similar_strings_differ_at_start() {
        // Ensure timing is consistent regardless of where difference occurs
        assert!(!constant_time_eq(b"Xest-token", b"test-token"));
    }

    #[test]
    fn test_constant_time_eq_similar_strings_differ_at_end() {
        assert!(!constant_time_eq(b"test-tokeX", b"test-token"));
    }

    #[test]
    fn test_constant_time_eq_similar_strings_differ_in_middle() {
        assert!(!constant_time_eq(b"test-Xoken", b"test-token"));
    }
}
