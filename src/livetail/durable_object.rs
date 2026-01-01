//! LiveTailDO: Durable Object with WebSocket hibernation for live streaming.

#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use worker::*;

/// Message types sent to WebSocket clients.
#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Connection established.
    Connected { message: String },
    /// A telemetry record.
    Record { data: serde_json::Value },
    /// Records were dropped due to rate limiting.
    Dropped { count: usize },
}

/// LiveTailDO: Streams records to connected WebSocket clients.
///
/// One DO per {service}:{signal} (e.g., "payment-service:logs").
/// Uses WebSocket hibernation for zero cost when unused.
#[cfg(target_arch = "wasm32")]
#[durable_object]
pub struct LiveTailDO {
    state: State,
    #[allow(dead_code)]
    env: Env,
}

#[cfg(target_arch = "wasm32")]
impl DurableObject for LiveTailDO {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();
        match (req.method(), path.as_str()) {
            (Method::Get, "/websocket") => self.handle_websocket_upgrade().await,
            (Method::Post, "/ingest") => self.handle_ingest(req).await,
            (Method::Get, "/status") => self.handle_status().await,
            _ => Response::error("Not found", 404),
        }
    }

    async fn websocket_message(
        &self,
        _ws: WebSocket,
        _message: WebSocketIncomingMessage,
    ) -> Result<()> {
        // Read-only stream, ignore client messages
        Ok(())
    }

    async fn websocket_close(
        &self,
        _ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        // get_websockets() automatically excludes closed sockets
        Ok(())
    }

    async fn websocket_error(&self, _ws: WebSocket, error: Error) -> Result<()> {
        console_warn!("WebSocket error: {:?}", error);
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl LiveTailDO {
    /// Maximum records per ingest batch to prevent flooding clients.
    const MAX_RECORDS_PER_BATCH: usize = 100;

    /// Handle WebSocket upgrade request.
    async fn handle_websocket_upgrade(&self) -> Result<Response> {
        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept the WebSocket with hibernation enabled
        self.state.accept_web_socket(&server);

        // Send welcome message
        let welcome = WsMessage::Connected {
            message: "Live tail stream started".to_string(),
        };
        let welcome_json = serde_json::to_string(&welcome)
            .map_err(|e| Error::RustError(format!("JSON error: {}", e)))?;
        server.send_with_str(&welcome_json)?;

        Response::from_websocket(client)
    }

    /// Handle record ingestion from workers.
    async fn handle_ingest(&self, mut req: Request) -> Result<Response> {
        let sockets = self.state.get_websockets();
        let client_count = sockets.len();

        // Early exit if no clients - return 0 so sender can cache
        if client_count == 0 {
            return Response::ok("0");
        }

        // Parse records
        let body = req.text().await?;
        let records: Vec<serde_json::Value> = serde_json::from_str(&body)
            .map_err(|e| Error::RustError(format!("Invalid JSON: {}", e)))?;

        if records.is_empty() {
            return Response::ok(format!("{}", client_count));
        }

        // Rate limit: cap records per batch
        let (to_send, dropped) = if records.len() > Self::MAX_RECORDS_PER_BATCH {
            (
                &records[..Self::MAX_RECORDS_PER_BATCH],
                records.len() - Self::MAX_RECORDS_PER_BATCH,
            )
        } else {
            (records.as_slice(), 0)
        };

        // Broadcast to all clients
        for ws in &sockets {
            // Send records
            for record in to_send {
                let msg = WsMessage::Record {
                    data: record.clone(),
                };
                match serde_json::to_string(&msg) {
                    Ok(json) => {
                        if let Err(e) = ws.send_with_str(&json) {
                            tracing::debug!(error = %e, "failed to send WebSocket message");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to serialize record for WebSocket");
                    }
                }
            }

            // Notify about dropped records
            if dropped > 0 {
                let drop_msg = WsMessage::Dropped { count: dropped };
                match serde_json::to_string(&drop_msg) {
                    Ok(json) => {
                        if let Err(e) = ws.send_with_str(&json) {
                            tracing::debug!(error = %e, "failed to send dropped notification");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to serialize dropped notification");
                    }
                }
            }
        }

        Response::ok(format!("{}", client_count))
    }

    /// Return current client count (for debugging/monitoring).
    async fn handle_status(&self) -> Result<Response> {
        let count = self.state.get_websockets().len();
        Response::from_json(&serde_json::json!({
            "clients": count
        }))
    }
}
