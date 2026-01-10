//! Firehose client implementing PipelineSender trait.

use crate::pipeline::{PipelineSender, SendResult};
use std::collections::HashMap;
use vrl::value::Value;

/// Firehose delivery stream configuration per signal type.
#[derive(Clone)]
pub struct StreamConfig {
    pub logs: String,
    pub traces: String,
    pub sum: String,
    pub gauge: String,
}

impl StreamConfig {
    /// Load stream names from environment variables.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            logs: std::env::var("PIPELINE_LOGS")?,
            traces: std::env::var("PIPELINE_TRACES")?,
            sum: std::env::var("PIPELINE_SUM")?,
            gauge: std::env::var("PIPELINE_GAUGE")?,
        })
    }

    /// Get stream name for a table.
    pub fn stream_for_table(&self, table: &str) -> Option<&str> {
        match table {
            "logs" => Some(&self.logs),
            "traces" => Some(&self.traces),
            "sum" => Some(&self.sum),
            "gauge" => Some(&self.gauge),
            _ => None,
        }
    }
}

/// Firehose client that implements PipelineSender.
pub struct FirehoseSender {
    client: aws_sdk_firehose::Client,
    streams: StreamConfig,
}

impl FirehoseSender {
    /// Create a new FirehoseSender from AWS config.
    pub async fn new(streams: StreamConfig) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Self {
            client: aws_sdk_firehose::Client::new(&config),
            streams,
        }
    }
}

#[async_trait::async_trait]
impl PipelineSender for FirehoseSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult {
        // Stub - will implement in next task
        let _ = (grouped, &self.client, &self.streams);
        SendResult::default()
    }
}
