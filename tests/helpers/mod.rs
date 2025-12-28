#![allow(dead_code)] // Test helpers appear unused when compiled independently

use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

const WAIT_ATTEMPTS: usize = 50;
const WAIT_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone)]
struct PipelineState {
    events: Arc<Mutex<Vec<Value>>>,
}

pub struct MockPipeline {
    shutdown_tx: oneshot::Sender<()>,
    handle: JoinHandle<()>,
}

impl MockPipeline {
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.handle.await;
    }
}

/// Find an available TCP port
pub async fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Best-effort check for whether binding to loopback is permitted in the current sandbox.
pub async fn can_bind_loopback() -> bool {
    match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => {
            drop(listener);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => false,
        Err(_) => true, // treat other errors as non-fatal for skipping
    }
}

/// Spawn the mock pipeline, return (process handle, base URL)
pub async fn spawn_mock_pipeline(port: u16) -> (MockPipeline, String) {
    let state = PipelineState {
        events: Arc::new(Mutex::new(Vec::new())),
    };

    let app_state = state.clone();
    let app = Router::new()
        .route("/", post(ingest))
        .route("/events", get(events))
        .route("/reset", post(reset))
        .route("/health", get(health))
        .with_state(app_state);

    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("failed to bind mock pipeline listener");

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            eprintln!("mock pipeline server error: {}", err);
        }
    });

    (
        MockPipeline {
            shutdown_tx,
            handle,
        },
        format!("http://127.0.0.1:{}", port),
    )
}

/// Wait for a server to respond to /health
pub async fn wait_for_health(client: &Client, base_url: &str) {
    poll_until(|| async {
        client
            .get(format!("{}/health", base_url))
            .send()
            .await
            .ok()
            .map(|_| ())
    })
    .await
    .unwrap_or_else(|| panic!("timed out waiting for {} to be healthy", base_url));
}

/// Poll /events until we have at least `min_count` events
pub async fn wait_for_events(
    client: &Client,
    base_url: &str,
    min_count: usize,
) -> Vec<serde_json::Value> {
    poll_until(|| async {
        match client.get(format!("{}/events", base_url)).send().await.ok() {
            Some(resp) => match resp.json::<Vec<serde_json::Value>>().await.ok() {
                Some(events) if events.len() >= min_count => Some(events),
                _ => None,
            },
            None => None,
        }
    })
    .await
    .unwrap_or_else(|| panic!("timed out waiting for {} events at {}", min_count, base_url))
}

/// Reset the mock pipeline's event store
pub async fn reset_events(client: &Client, base_url: &str) {
    client
        .post(format!("{}/reset", base_url))
        .send()
        .await
        .expect("failed to reset events");
}

async fn poll_until<T, F, Fut>(mut f: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    for _ in 0..WAIT_ATTEMPTS {
        if let Some(result) = f().await {
            return Some(result);
        }
        tokio::time::sleep(WAIT_DELAY).await;
    }
    None
}

async fn ingest(
    State(state): State<PipelineState>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let text = String::from_utf8_lossy(&body);
    let mut events = state.events.lock().await;
    for line in text.split('\n').filter(|l| !l.is_empty()) {
        let parsed: Value = serde_json::from_str(line).map_err(|_| StatusCode::BAD_REQUEST)?;
        events.push(parsed);
    }

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn events(State(state): State<PipelineState>) -> Json<Vec<Value>> {
    let events = state.events.lock().await;
    Json(events.clone())
}

async fn reset(State(state): State<PipelineState>) -> Json<serde_json::Value> {
    let mut events = state.events.lock().await;
    events.clear();
    Json(serde_json::json!({ "status": "ok" }))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
