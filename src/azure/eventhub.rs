//! Event Hub client implementing PipelineSender trait.

use azeventhubs::{
    producer::{EventHubProducerClient, EventHubProducerClientOptions, SendEventOptions},
    BasicRetryPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::pipeline::{PipelineSender, SendResult};

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

/// Event envelope wrapping transformed records for Stream Analytics routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventEnvelope {
    signal_type: String,
    table: String,
    payload: Value,
}

impl EventEnvelope {
    fn new(table: &str, payload: Value) -> Self {
        // Map table name to signal_type for Stream Analytics routing
        let signal_type = match table {
            "logs" => "logs",
            "traces" => "traces",
            "gauge" => "metrics_gauge",
            "sum" => "metrics_sum",
            _ => table,
        };
        Self {
            signal_type: signal_type.to_string(),
            table: table.to_string(),
            payload,
        }
    }
}

/// Event Hub sender that implements PipelineSender.
pub struct EventHubSender {
    producer: Mutex<EventHubProducerClient<BasicRetryPolicy>>,
}

impl EventHubSender {
    /// Create a new EventHubSender.
    pub async fn new(config: EventHubConfig) -> Result<Self, String> {
        let producer = EventHubProducerClient::new_from_connection_string(
            &config.connection_string,
            config.hub_name,
            EventHubProducerClientOptions::default(),
        )
        .await
        .map_err(|e| format!("Failed to create Event Hub producer: {:?}", e))?;

        Ok(Self {
            producer: Mutex::new(producer),
        })
    }

    async fn send_batch(&self, table: &str, records: Vec<Value>) -> Result<usize, String> {
        let count = records.len();
        let mut producer = self.producer.lock().await;

        for record in records {
            let envelope = EventEnvelope::new(table, record);
            let json = serde_json::to_vec(&envelope)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;

            producer
                .send_event(json, SendEventOptions::default())
                .await
                .map_err(|e| format!("Event Hub send failed: {:?}", e))?;
        }

        debug!(table = table, count = count, "Sent events to Event Hub");
        Ok(count)
    }
}

#[async_trait::async_trait]
impl PipelineSender for EventHubSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult {
        let mut result = SendResult::default();

        for (table, records) in grouped {
            match self.send_batch(&table, records).await {
                Ok(count) => {
                    result.succeeded.insert(table, count);
                }
                Err(e) => {
                    error!(table = %table, error = %e, "Failed to send to Event Hub");
                    result.failed.insert(table, e);
                }
            }
        }

        result
    }
}
