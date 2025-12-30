// tests/e2e_traces.rs
mod helpers;

use helpers::{
    can_bind_loopback, free_port, reset_events, spawn_mock_pipeline, wait_for_events,
    wait_for_health,
};
use otlpflare::Signal;
use reqwest::Client;
use std::collections::HashMap;

#[tokio::test]
async fn test_otlp_traces_flow() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e traces test (json): cannot bind to loopback in this environment");
        return;
    }

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start otlpflare router with spans endpoint
    let app_port = free_port().await;
    let mut endpoints = HashMap::new();
    endpoints.insert(Signal::Traces, mock_url.clone());
    let app = otlpflare::native::build_router_multi(endpoints);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for app to be ready
    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Send OTLP traces
    let otlp_payload = include_str!("fixtures/sample_otlp_traces.json");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/traces", app_port))
        .header("content-type", "application/json")
        .body(otlp_payload)
        .send()
        .await
        .expect("failed to send request");

    assert!(
        resp.status().is_success(),
        "response was not success: {:?}",
        resp.status()
    );

    // 4. Verify events arrived at mock pipeline
    let events = wait_for_events(&client, &mock_url, 1).await;
    assert!(!events.is_empty(), "no events received");

    // Verify expected span fields exist
    let first = &events[0];
    assert!(first.get("trace_id").is_some(), "missing trace_id");
    assert!(first.get("span_id").is_some(), "missing span_id");
    assert!(first.get("span_name").is_some(), "missing span_name");
    assert!(first.get("span_kind").is_some(), "missing span_kind");
    assert!(first.get("duration").is_some(), "missing duration");
    assert!(first.get("status_code").is_some(), "missing status_code");
    assert!(first.get("service_name").is_some(), "missing service_name");

    // Verify events and links are present (as JSON strings)
    assert!(first.get("events_json").is_some(), "missing events_json");
    assert!(first.get("links_json").is_some(), "missing links_json");

    // Verify duration is correctly converted from nanoseconds to milliseconds
    // Input: startTimeUnixNano=1703265600000000000, endTimeUnixNano=1703265600100000000
    // Duration: 100,000,000 ns = 100 ms
    assert_eq!(
        first.get("duration").and_then(|v| v.as_i64()),
        Some(100),
        "duration should be 100ms (VRL converts from nanoseconds to milliseconds)"
    );

    // Verify new dropped count and flags fields from JSON input
    assert_eq!(
        first
            .get("dropped_attributes_count")
            .and_then(|v| v.as_i64()),
        Some(7),
        "dropped_attributes_count should be 7"
    );
    assert_eq!(
        first.get("dropped_events_count").and_then(|v| v.as_i64()),
        Some(4),
        "dropped_events_count should be 4"
    );
    assert_eq!(
        first.get("dropped_links_count").and_then(|v| v.as_i64()),
        Some(1),
        "dropped_links_count should be 1"
    );
    assert_eq!(
        first.get("flags").and_then(|v| v.as_i64()),
        Some(1),
        "flags should be 1 (sampled)"
    );

    // 5. Cleanup
    mock_proc.stop().await;
}

#[tokio::test]
async fn test_otlp_traces_protobuf() {
    if !can_bind_loopback().await {
        eprintln!(
            "skipping e2e traces test (protobuf): cannot bind to loopback in this environment"
        );
        return;
    }

    use opentelemetry_proto::tonic::{
        collector::trace::v1::ExportTraceServiceRequest,
        common::v1::{any_value, AnyValue, InstrumentationScope, KeyValue},
        resource::v1::Resource,
        trace::v1::{span, ResourceSpans, ScopeSpans, Span, Status},
    };
    use prost::Message;

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start otlpflare router
    let app_port = free_port().await;
    let mut endpoints = HashMap::new();
    endpoints.insert(Signal::Traces, mock_url.clone());
    let app = otlpflare::native::build_router_multi(endpoints);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Create and send protobuf payload
    let span = Span {
        trace_id: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        span_id: vec![0, 1, 2, 3, 4, 5, 6, 7],
        parent_span_id: vec![],
        name: "test-span".to_string(),
        kind: span::SpanKind::Server as i32,
        start_time_unix_nano: 1_000_000_000,
        end_time_unix_nano: 2_000_000_000,
        attributes: vec![KeyValue {
            key: "test.attr".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue("test-value".to_string())),
            }),
        }],
        status: Some(Status {
            code: 1,
            message: "OK".to_string(),
        }),
        dropped_attributes_count: 5,
        dropped_events_count: 3,
        dropped_links_count: 2,
        flags: 1, // W3C trace flag: sampled
        ..Default::default()
    };

    let request = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(Resource {
                attributes: vec![KeyValue {
                    key: "service.name".to_string(),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::StringValue("test-service".to_string())),
                    }),
                }],
                ..Default::default()
            }),
            scope_spans: vec![ScopeSpans {
                scope: Some(InstrumentationScope {
                    name: "test-lib".to_string(),
                    version: "1.0.0".to_string(),
                    ..Default::default()
                }),
                spans: vec![span],
                ..Default::default()
            }],
            ..Default::default()
        }],
    };

    let body = request.encode_to_vec();

    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/traces", app_port))
        .header("content-type", "application/x-protobuf")
        .body(body)
        .send()
        .await
        .expect("failed to send request");

    assert!(
        resp.status().is_success(),
        "response was not success: {:?}",
        resp.status()
    );

    // 4. Verify span arrived
    let events = wait_for_events(&client, &mock_url, 1).await;
    assert!(!events.is_empty(), "no events received");

    let first = &events[0];
    assert_eq!(
        first.get("span_name").and_then(|v| v.as_str()),
        Some("test-span")
    );
    assert_eq!(
        first.get("service_name").and_then(|v| v.as_str()),
        Some("test-service")
    );

    // Verify new dropped count and flags fields
    assert_eq!(
        first
            .get("dropped_attributes_count")
            .and_then(|v| v.as_i64()),
        Some(5),
        "dropped_attributes_count should be 5"
    );
    assert_eq!(
        first.get("dropped_events_count").and_then(|v| v.as_i64()),
        Some(3),
        "dropped_events_count should be 3"
    );
    assert_eq!(
        first.get("dropped_links_count").and_then(|v| v.as_i64()),
        Some(2),
        "dropped_links_count should be 2"
    );
    assert_eq!(
        first.get("flags").and_then(|v| v.as_i64()),
        Some(1),
        "flags should be 1 (sampled)"
    );

    // 5. Cleanup
    mock_proc.stop().await;
}
