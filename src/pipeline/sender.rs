// src/pipeline/sender.rs
use std::collections::HashMap;
use vrl::value::Value;

/// Result of sending to multiple pipelines
#[derive(Debug, Default)]
pub struct SendResult {
    pub succeeded: HashMap<String, usize>,
    pub failed: HashMap<String, String>,
}

/// Trait for sending batches to pipelines (abstracts HTTP client)
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait PipelineSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult;
}
