// tests/e2e_hec_logs.rs
mod helpers;

use helpers::{
    can_bind_loopback, free_port, reset_events, spawn_mock_pipeline, wait_for_events,
    wait_for_health,
};
use reqwest::Client;

#[tokio::test]
async fn test_hec_logs_flow() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e HEC logs test: cannot bind to loopback in this environment");
        return;
    }

    let client = Client::new();

    // 1. Start mock pipeline
    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    // 2. Start otlp2pipeline router
    let app_port = free_port().await;
    let app = otlp2pipeline::build_router(mock_url.clone());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for app to be ready
    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // 3. Send HEC logs (NDJSON format)
    let hec_payload = include_str!("fixtures/sample_hec.json");
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/services/collector/event",
            app_port
        ))
        .header("content-type", "application/json")
        .body(hec_payload)
        .send()
        .await
        .expect("failed to send request");

    assert!(
        resp.status().is_success(),
        "response was not success: {:?}",
        resp.status()
    );

    // 4. Verify events arrived at mock pipeline
    // Fixture contains 5 real-world HEC events from:
    // - OpenTelemetry collector-contrib tests
    // - Docker logging plugin format
    // - Kubernetes container logs
    // - AWS EC2 instance logs
    // - Fluentd forwarder
    let events = wait_for_events(&client, &mock_url, 5).await;
    assert_eq!(events.len(), 5, "expected 5 events");

    // Verify expected fields exist in transformed output
    let first = &events[0];
    assert!(first.get("timestamp").is_some(), "missing timestamp");
    assert!(first.get("body").is_some(), "missing body");
    assert!(first.get("service_name").is_some(), "missing service_name");

    // First event (OTel test pattern) has no host, so service_name = "unknown"
    assert_eq!(first.get("service_name").unwrap().as_str(), Some("unknown"));

    // Second event (Docker container log) has host
    let docker_event = &events[1];
    assert_eq!(
        docker_event.get("service_name").unwrap().as_str(),
        Some("docker-host-1")
    );

    // Verify resource_attributes contains host.name for Docker event
    let resource_attrs = docker_event
        .get("resource_attributes")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(
        resource_attrs.contains("host.name"),
        "resource_attributes should contain host.name"
    );

    // Third event (Kubernetes log) has structured metadata
    let k8s_event = &events[2];
    let k8s_attrs = k8s_event.get("log_attributes").unwrap().as_str().unwrap();
    assert!(
        k8s_attrs.contains("cluster_name"),
        "k8s log should have cluster_name in log_attributes"
    );

    // Fourth event (AWS) has structured event body
    let aws_event = &events[3];
    let body = aws_event.get("body").unwrap().as_str().unwrap();
    assert!(
        body.contains("Something happened"),
        "structured event body should be JSON-encoded"
    );

    // 5. Cleanup
    mock_proc.stop().await;
}

#[tokio::test]
async fn test_hec_logs_single_event() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e HEC single event test: cannot bind to loopback");
        return;
    }

    let client = Client::new();

    let mock_port = free_port().await;
    let (mock_proc, mock_url) = spawn_mock_pipeline(mock_port).await;
    wait_for_health(&client, &mock_url).await;
    reset_events(&client, &mock_url).await;

    let app_port = free_port().await;
    let app = otlp2pipeline::build_router(mock_url.clone());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", app_port))
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    wait_for_health(&client, &format!("http://127.0.0.1:{}", app_port)).await;

    // Send single event (not NDJSON)
    let single_event = r#"{"time": 1703265600, "host": "test-host", "event": "single log line"}"#;
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/services/collector/event",
            app_port
        ))
        .body(single_event)
        .send()
        .await
        .expect("failed to send request");

    assert!(resp.status().is_success());

    let events = wait_for_events(&client, &mock_url, 1).await;
    assert_eq!(events.len(), 1);

    mock_proc.stop().await;
}
