//! Event Hub client implementing PipelineSender trait.

use crate::pipeline::{PipelineSender, SendResult};
use serde_json::Value;
use std::collections::HashMap;

/// Event Hub configuration loaded from environment.
#[derive(Clone)]
pub struct EventHubConfig {
    pub connection_string: String,
    pub hub_name: String,
}

impl EventHubConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            connection_string: std::env::var("EVENTHUB_CONNECTION_STRING")
                .map_err(|_| "EVENTHUB_CONNECTION_STRING not set")?,
            hub_name: std::env::var("EVENTHUB_NAME")
                .unwrap_or_else(|_| "otlp-ingestion".to_string()),
        })
    }
}

/// Event Hub sender that implements PipelineSender.
pub struct EventHubSender {
    _config: EventHubConfig,
}

impl EventHubSender {
    /// Create a new EventHubSender (placeholder).
    pub async fn new(config: EventHubConfig) -> Result<Self, String> {
        Ok(Self { _config: config })
    }
}

#[async_trait::async_trait]
impl PipelineSender for EventHubSender {
    async fn send_all(&self, _grouped: HashMap<String, Vec<Value>>) -> SendResult {
        // TODO: Implement actual Event Hub sending
        SendResult::default()
    }
}
