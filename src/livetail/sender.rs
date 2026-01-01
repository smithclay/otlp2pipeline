//! LiveTailSender trait and implementations.

use std::collections::HashMap;
use vrl::value::Value;

/// Result of sending to livetail DOs.
#[derive(Debug, Default)]
pub struct LiveTailSendResult {
    /// Number of records sent per DO.
    pub sent: HashMap<String, usize>,
    /// Errors per DO (best-effort, logged but not fatal).
    pub errors: HashMap<String, String>,
}

impl LiveTailSendResult {
    /// Create a disabled result (feature flag off).
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Create a success result.
    pub fn ok() -> Self {
        Self::default()
    }
}

/// Trait for sending records to LiveTailDO instances.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait LiveTailSender {
    /// Send grouped records to relevant LiveTailDOs.
    ///
    /// Records are grouped by table name (logs, traces).
    /// Each record contains service_name for DO routing.
    async fn send_to_livetail(&self, grouped: HashMap<String, Vec<Value>>) -> LiveTailSendResult;
}

/// NoOp implementation for native builds (testing).
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeLiveTailSender;

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeLiveTailSender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeLiveTailSender {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl LiveTailSender for NativeLiveTailSender {
    async fn send_to_livetail(&self, _grouped: HashMap<String, Vec<Value>>) -> LiveTailSendResult {
        // NoOp for native - livetail is a WASM-only feature
        LiveTailSendResult::disabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_result_defaults() {
        let result = LiveTailSendResult::default();
        assert!(result.sent.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_send_result_disabled() {
        let result = LiveTailSendResult::disabled();
        assert!(result.sent.is_empty());
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_native_sender_returns_disabled() {
        let sender = NativeLiveTailSender::new();
        let result = sender.send_to_livetail(HashMap::new()).await;
        assert!(result.sent.is_empty());
        assert!(result.errors.is_empty());
    }
}
