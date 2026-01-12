//! Lambda integration tests.
//! Run with: cargo test --features lambda --test lambda_integration

#![cfg(feature = "lambda")]

use bytes::Bytes;
use otlp2pipeline::{
    handle_signal, InputFormat, LogsHandler, PipelineSender, SendResult, TracesHandler,
};
use serde_json::Value;
use std::collections::HashMap;

/// Mock PipelineSender for testing.
struct MockSender {
    records: std::sync::Mutex<HashMap<String, Vec<Value>>>,
}

impl MockSender {
    fn new() -> Self {
        Self {
            records: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn get_records(&self) -> HashMap<String, Vec<Value>> {
        self.records.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl PipelineSender for MockSender {
    async fn send_all(&self, grouped: HashMap<String, Vec<Value>>) -> SendResult {
        let mut records = self.records.lock().unwrap();
        for (table, values) in grouped {
            records.entry(table).or_default().extend(values);
        }
        SendResult::default()
    }
}

/// Test that logs are correctly decoded and transformed.
#[tokio::test]
async fn test_logs_handler_json() {
    let json_payload = r#"{
        "resourceLogs": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": "test-service"}}
                ]
            },
            "scopeLogs": [{
                "logRecords": [{
                    "timeUnixNano": "1704067200000000000",
                    "severityNumber": 9,
                    "severityText": "INFO",
                    "body": {"stringValue": "Test log message"}
                }]
            }]
        }]
    }"#;

    let sender = MockSender::new();
    let result = handle_signal::<LogsHandler, _>(
        Bytes::from(json_payload),
        false,
        InputFormat::Json,
        &sender,
    )
    .await;

    assert!(result.is_ok(), "Handler failed: {:?}", result.err());

    let records = sender.get_records();
    assert!(records.contains_key("logs"), "No logs table in output");
    assert_eq!(records["logs"].len(), 1, "Expected 1 log record");
}

/// Test that traces are correctly decoded and transformed.
#[tokio::test]
async fn test_traces_handler_json() {
    let json_payload = r#"{
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": "test-service"}}
                ]
            },
            "scopeSpans": [{
                "spans": [{
                    "traceId": "0102030405060708090a0b0c0d0e0f10",
                    "spanId": "0102030405060708",
                    "name": "test-span",
                    "kind": 1,
                    "startTimeUnixNano": "1704067200000000000",
                    "endTimeUnixNano": "1704067200100000000",
                    "status": {"code": 1}
                }]
            }]
        }]
    }"#;

    let sender = MockSender::new();
    let result = handle_signal::<TracesHandler, _>(
        Bytes::from(json_payload),
        false,
        InputFormat::Json,
        &sender,
    )
    .await;

    assert!(result.is_ok(), "Handler failed: {:?}", result.err());

    let records = sender.get_records();
    assert!(records.contains_key("traces"), "No traces table in output");
    assert_eq!(records["traces"].len(), 1, "Expected 1 trace record");
}
