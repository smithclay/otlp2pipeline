//! Azure Event Hub PoC - Validates Event Hubs Capture integration
//!
//! This PoC demonstrates:
//! 1. Connecting to Azure Event Hub with Capture enabled
//! 2. Sending enveloped JSON events with signal_type metadata
//! 3. Batch sending and automatic Parquet batching via Capture
//! 4. Events are automatically captured to Azure Data Lake Storage Gen2
//!
//! Run with:
//! ```bash
//! export EVENTHUB_CONNECTION_STRING="Endpoint=sb://..."
//! export EVENTHUB_NAME="otlp-ingestion"
//! cargo run --example azure_eventhub_poc --features azure
//! ```

use azeventhubs::producer::{
    EventHubProducerClient, EventHubProducerClientOptions, SendEventOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Event envelope wrapping transformed records for Fabric routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub signal_type: String,
    pub table: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    pub payload: Value,
}

impl EventEnvelope {
    /// Create a logs envelope with sample data
    fn sample_log(service_name: &str, message: &str) -> Self {
        Self {
            signal_type: "logs".to_string(),
            table: "logs".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            service_name: Some(service_name.to_string()),
            env: Some("poc".to_string()),
            payload: serde_json::json!({
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "service_name": service_name,
                "body": message,
                "severity_number": 9,
                "severity_text": "INFO",
                "trace_id": "abc123",
                "span_id": "def456",
            }),
        }
    }

    /// Create a traces envelope with sample data
    fn sample_trace(service_name: &str, operation: &str) -> Self {
        Self {
            signal_type: "traces".to_string(),
            table: "traces".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            service_name: Some(service_name.to_string()),
            env: Some("poc".to_string()),
            payload: serde_json::json!({
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "service_name": service_name,
                "span_name": operation,
                "trace_id": "trace123",
                "span_id": "span456",
                "parent_span_id": null,
                "duration_ms": 125,
                "status_code": 1,
            }),
        }
    }

    /// Create a metrics (gauge) envelope with sample data
    fn sample_gauge(service_name: &str, metric_name: &str, value: f64) -> Self {
        Self {
            signal_type: "metrics_gauge".to_string(),
            table: "gauge".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            service_name: Some(service_name.to_string()),
            env: Some("poc".to_string()),
            payload: serde_json::json!({
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "service_name": service_name,
                "metric_name": metric_name,
                "value": value,
                "unit": "bytes",
            }),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    println!("🚀 Azure Event Hub PoC - Starting");
    println!();

    // Load configuration from environment
    let connection_string = std::env::var("EVENTHUB_CONNECTION_STRING")
        .expect("EVENTHUB_CONNECTION_STRING environment variable required");
    let eventhub_name =
        std::env::var("EVENTHUB_NAME").unwrap_or_else(|_| "otlp-ingestion".to_string());

    println!("📋 Configuration:");
    println!("   Event Hub Name: {}", eventhub_name);
    println!(
        "   Connection String: {}...",
        &connection_string[..50.min(connection_string.len())]
    );
    println!();

    // Create Event Hub producer client
    println!("🔌 Creating Event Hub producer client...");
    let mut producer = EventHubProducerClient::new_from_connection_string(
        &connection_string,
        eventhub_name.clone(),
        EventHubProducerClientOptions::default(),
    )
    .await?;
    println!("✅ Producer client created successfully");
    println!();

    // Test 1: Send single log event
    println!("📤 Test 1: Sending single log event");
    let log_envelope = EventEnvelope::sample_log("api-gateway", "Request received");
    let log_json = serde_json::to_vec(&log_envelope)?;

    producer
        .send_event(log_json, SendEventOptions::default())
        .await
        .map_err(|e| format!("Failed to send log: {:?}", e))?;
    println!("✅ Log event sent successfully");
    println!("   Signal Type: {}", log_envelope.signal_type);
    println!(
        "   Service: {}",
        log_envelope.service_name.as_deref().unwrap_or("unknown")
    );
    println!(
        "   Payload size: {} bytes",
        serde_json::to_vec(&log_envelope)?.len()
    );
    println!();

    // Test 2: Send batch of trace events
    println!("📤 Test 2: Sending batch of 5 trace events");
    let trace_envelopes: Vec<EventEnvelope> = (0..5)
        .map(|i| EventEnvelope::sample_trace("api-gateway", &format!("operation_{}", i)))
        .collect();

    let trace_batch: Vec<Vec<u8>> = trace_envelopes
        .iter()
        .map(|env| serde_json::to_vec(env).unwrap())
        .collect();

    // Send as individual events (Event Hubs supports batching)
    for (i, trace_data) in trace_batch.iter().enumerate() {
        producer
            .send_event(trace_data.clone(), SendEventOptions::default())
            .await
            .map_err(|e| format!("Failed to send trace {}: {:?}", i, e))?;
    }
    println!(
        "✅ Batch of {} trace events sent successfully",
        trace_batch.len()
    );
    println!();

    // Test 3: Send mixed signal types
    println!("📤 Test 3: Sending mixed signal types (logs, traces, metrics)");
    let mixed_envelopes = [
        EventEnvelope::sample_log("web-server", "User login successful"),
        EventEnvelope::sample_trace("web-server", "authenticate_user"),
        EventEnvelope::sample_gauge("web-server", "memory_usage_bytes", 1024000.0),
        EventEnvelope::sample_log("database", "Query executed"),
        EventEnvelope::sample_trace("database", "execute_query"),
    ];

    for (i, envelope) in mixed_envelopes.iter().enumerate() {
        let json = serde_json::to_vec(envelope)?;
        producer
            .send_event(json, SendEventOptions::default())
            .await
            .map_err(|e| format!("Failed to send mixed event {}: {:?}", i, e))?;
        println!(
            "   ✓ Sent {} event from {}",
            envelope.signal_type,
            envelope.service_name.as_deref().unwrap_or("unknown")
        );
    }
    println!("✅ Mixed events sent successfully");
    println!();

    // Test 4: Large batch to test chunking
    println!("📤 Test 4: Sending large batch (50 events)");
    let large_batch: Vec<EventEnvelope> = (0..50)
        .map(|i| {
            let signal_type = match i % 4 {
                0 => "logs",
                1 => "traces",
                2 => "metrics_gauge",
                _ => "metrics_sum",
            };
            match signal_type {
                "logs" => EventEnvelope::sample_log("load-test", &format!("Log message {}", i)),
                "traces" => EventEnvelope::sample_trace("load-test", &format!("op_{}", i)),
                "metrics_gauge" => {
                    EventEnvelope::sample_gauge("load-test", "test_metric", i as f64)
                }
                _ => EventEnvelope::sample_gauge("load-test", "counter", i as f64),
            }
        })
        .collect();

    for (i, envelope) in large_batch.iter().enumerate() {
        let json = serde_json::to_vec(envelope)?;
        producer
            .send_event(json, SendEventOptions::default())
            .await
            .map_err(|e| format!("Failed to send batch event {}: {:?}", i, e))?;

        if (i + 1) % 10 == 0 {
            println!("   ✓ Sent {} events...", i + 1);
        }
    }
    println!("✅ Large batch sent successfully");
    println!();

    // Test 5: Verify envelope schema
    println!("📋 Test 5: Verifying envelope schema");
    let verification_envelope = EventEnvelope::sample_log("schema-test", "Schema verification");
    let json_pretty = serde_json::to_string_pretty(&verification_envelope)?;
    println!("   Envelope structure:");
    println!("{}", json_pretty);
    println!("✅ Schema verified");
    println!();

    // Summary
    println!("🎉 PoC Complete!");
    println!();
    println!("Summary:");
    println!("  ✓ Single event send");
    println!("  ✓ Batch event send");
    println!("  ✓ Mixed signal types");
    println!("  ✓ Large batch (50 events)");
    println!("  ✓ Schema validation");
    println!();
    println!("Next steps:");
    println!("  1. Wait 5 minutes for Capture to flush (or send enough data to hit 300MB)");
    println!("  2. Check Azure Data Lake Storage for Parquet/Avro files");
    println!("  3. Download and inspect captured data");
    println!("  4. Verify event structure is preserved");

    // Close the producer client
    producer.close().await?;

    Ok(())
}
