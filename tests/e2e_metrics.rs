// tests/e2e_metrics.rs
mod helpers;

use helpers::{
    can_bind_loopback, free_port, reset_events, spawn_mock_pipeline, wait_for_events,
    wait_for_health,
};
use reqwest::Client;

#[tokio::test]
async fn test_otlp_metrics_flow() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e metrics test: cannot bind to loopback in this environment");
        return;
    }

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start frostbit router
    let app_port = free_port().await;
    let app = frostbit::build_router(mock_url.clone());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for app to be ready
    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Send OTLP metrics
    let otlp_payload = include_str!("fixtures/sample_otlp_metrics.json");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/metrics", app_port))
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

    // 4. Verify events arrived at mock pipeline (should have 3: 2 gauges + 1 sum)
    let events = wait_for_events(&client, &mock_url, 3).await;
    assert_eq!(events.len(), 3, "expected 3 events (2 gauges + 1 sum)");

    // Find gauge and sum events
    let gauge_float_event = events.iter().find(|e| {
        e.get("metric_name")
            .and_then(|v| v.as_str())
            .map(|s| s == "cpu.usage")
            .unwrap_or(false)
    });
    let gauge_int_event = events.iter().find(|e| {
        e.get("metric_name")
            .and_then(|v| v.as_str())
            .map(|s| s == "active.connections")
            .unwrap_or(false)
    });
    let sum_event = events.iter().find(|e| {
        e.get("metric_name")
            .and_then(|v| v.as_str())
            .map(|s| s == "http.requests")
            .unwrap_or(false)
    });

    // Verify gauge event (asDouble input)
    assert!(gauge_float_event.is_some(), "missing cpu.usage gauge event");
    let gauge = gauge_float_event.unwrap();
    assert!(gauge.get("timestamp").is_some(), "missing timestamp");
    assert!(gauge.get("value").is_some(), "missing value");
    assert!(gauge.get("service_name").is_some(), "missing service_name");
    // Verify value is a float
    assert!(
        gauge.get("value").unwrap().is_f64(),
        "cpu.usage value should be f64"
    );

    // Verify gauge event (asInt input) - value should still be serialized as float
    assert!(
        gauge_int_event.is_some(),
        "missing active.connections gauge event"
    );
    let gauge_int = gauge_int_event.unwrap();
    assert!(gauge_int.get("value").is_some(), "missing value");
    // Critical: even though input was asInt, output must be float for Cloudflare schema
    let int_value = gauge_int.get("value").unwrap();
    assert!(
        int_value.is_f64(),
        "active.connections value should be f64, got: {:?}",
        int_value
    );
    assert_eq!(
        int_value.as_f64().unwrap(),
        42.0,
        "expected value 42.0 from asInt input"
    );

    // Verify sum event
    assert!(sum_event.is_some(), "missing sum event");
    let sum = sum_event.unwrap();
    assert!(
        sum.get("aggregation_temporality").is_some(),
        "missing aggregation_temporality"
    );
    assert!(sum.get("is_monotonic").is_some(), "missing is_monotonic");
    // Sum value should also be float
    assert!(
        sum.get("value").unwrap().is_f64(),
        "http.requests value should be f64"
    );

    // 5. Cleanup
    mock_proc.stop().await;
}

/// Test with real-world gauge protobuf fixture captured from production
#[tokio::test]
async fn test_otlp_metrics_gauge_protobuf() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e gauge protobuf test: cannot bind to loopback");
        return;
    }

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start frostbit router
    let app_port = free_port().await;
    let app = frostbit::build_router(mock_url.clone());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Send real-world gauge protobuf
    let otlp_payload = include_bytes!("fixtures/metrics_gauge.pb");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/metrics", app_port))
        .header("content-type", "application/x-protobuf")
        .body(otlp_payload.to_vec())
        .send()
        .await
        .expect("failed to send request");

    assert!(
        resp.status().is_success(),
        "response was not success: {:?}",
        resp.status()
    );

    // 4. Verify events arrived
    let events = wait_for_events(&client, &mock_url, 1).await;

    // Print actual JSON for debugging production issues
    for (i, event) in events.iter().enumerate() {
        eprintln!("=== Gauge Protobuf Event {} ===", i);
        eprintln!("{}", serde_json::to_string_pretty(event).unwrap());
    }

    assert!(!events.is_empty(), "expected at least 1 gauge event");

    // Verify required fields exist
    let event = &events[0];
    assert!(event.get("timestamp").is_some(), "missing timestamp");
    assert!(event.get("metric_name").is_some(), "missing metric_name");
    assert!(event.get("value").is_some(), "missing value");
    assert!(event.get("service_name").is_some(), "missing service_name");

    // Verify value is a float
    assert!(event.get("value").unwrap().is_f64(), "value should be f64");

    // Verify timestamp is an integer
    assert!(
        event.get("timestamp").unwrap().is_i64(),
        "timestamp should be i64"
    );

    mock_proc.stop().await;
}

/// Test with real-world sum protobuf fixture captured from production
#[tokio::test]
async fn test_otlp_metrics_sum_protobuf() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e sum protobuf test: cannot bind to loopback");
        return;
    }

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start frostbit router
    let app_port = free_port().await;
    let app = frostbit::build_router(mock_url.clone());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Send real-world sum protobuf
    let otlp_payload = include_bytes!("fixtures/metrics_sum.pb");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/metrics", app_port))
        .header("content-type", "application/x-protobuf")
        .body(otlp_payload.to_vec())
        .send()
        .await
        .expect("failed to send request");

    assert!(
        resp.status().is_success(),
        "response was not success: {:?}",
        resp.status()
    );

    // 4. Verify events arrived
    let events = wait_for_events(&client, &mock_url, 1).await;

    // Print actual JSON for debugging production issues
    for (i, event) in events.iter().enumerate() {
        eprintln!("=== Sum Protobuf Event {} ===", i);
        eprintln!("{}", serde_json::to_string_pretty(event).unwrap());
    }

    assert!(!events.is_empty(), "expected at least 1 sum event");

    // Verify required fields exist
    let event = &events[0];
    assert!(event.get("timestamp").is_some(), "missing timestamp");
    assert!(event.get("metric_name").is_some(), "missing metric_name");
    assert!(event.get("value").is_some(), "missing value");
    assert!(event.get("service_name").is_some(), "missing service_name");
    assert!(
        event.get("aggregation_temporality").is_some(),
        "missing aggregation_temporality"
    );
    assert!(event.get("is_monotonic").is_some(), "missing is_monotonic");

    // Verify value is a float
    assert!(event.get("value").unwrap().is_f64(), "value should be f64");

    // Verify timestamp is an integer
    assert!(
        event.get("timestamp").unwrap().is_i64(),
        "timestamp should be i64"
    );

    mock_proc.stop().await;
}

/// Test with mixed gauge+sum protobuf fixture - verifies routing to multiple pipelines
#[tokio::test]
async fn test_otlp_metrics_mixed_protobuf() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e mixed protobuf test: cannot bind to loopback");
        return;
    }

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start frostbit router
    let app_port = free_port().await;
    let app = frostbit::build_router(mock_url.clone());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Send mixed gauge+sum protobuf
    let otlp_payload = include_bytes!("fixtures/metrics_mixed.pb");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/metrics", app_port))
        .header("content-type", "application/x-protobuf")
        .body(otlp_payload.to_vec())
        .send()
        .await
        .expect("failed to send request");

    assert!(
        resp.status().is_success(),
        "response was not success: {:?}",
        resp.status()
    );

    // 4. Verify events arrived - expect both gauge and sum events
    let events = wait_for_events(&client, &mock_url, 2).await;

    // Print actual JSON for debugging
    for (i, event) in events.iter().enumerate() {
        eprintln!("=== Mixed Protobuf Event {} ===", i);
        eprintln!("{}", serde_json::to_string_pretty(event).unwrap());
    }

    assert!(
        events.len() >= 2,
        "expected at least 2 events (gauge + sum), got {}",
        events.len()
    );

    // Separate gauge and sum events by checking for sum-specific fields
    let gauge_events: Vec<_> = events
        .iter()
        .filter(|e| e.get("aggregation_temporality").is_none())
        .collect();
    let sum_events: Vec<_> = events
        .iter()
        .filter(|e| e.get("aggregation_temporality").is_some())
        .collect();

    assert!(
        !gauge_events.is_empty(),
        "expected at least 1 gauge event, got 0"
    );
    assert!(
        !sum_events.is_empty(),
        "expected at least 1 sum event, got 0"
    );

    // Verify gauge event structure
    let gauge = gauge_events[0];
    assert!(gauge.get("timestamp").is_some(), "gauge missing timestamp");
    assert!(
        gauge.get("metric_name").is_some(),
        "gauge missing metric_name"
    );
    assert!(gauge.get("value").is_some(), "gauge missing value");
    assert!(
        gauge.get("value").unwrap().is_f64(),
        "gauge value should be f64"
    );

    // Verify sum event structure
    let sum = sum_events[0];
    assert!(sum.get("timestamp").is_some(), "sum missing timestamp");
    assert!(sum.get("metric_name").is_some(), "sum missing metric_name");
    assert!(sum.get("value").is_some(), "sum missing value");
    assert!(
        sum.get("aggregation_temporality").is_some(),
        "sum missing aggregation_temporality"
    );
    assert!(
        sum.get("is_monotonic").is_some(),
        "sum missing is_monotonic"
    );
    assert!(
        sum.get("value").unwrap().is_f64(),
        "sum value should be f64"
    );

    mock_proc.stop().await;
}
