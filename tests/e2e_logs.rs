// tests/e2e_logs.rs
mod helpers;

use helpers::{
    can_bind_loopback, free_port, reset_events, spawn_mock_pipeline, wait_for_events,
    wait_for_health,
};
use reqwest::Client;

#[tokio::test]
async fn test_otlp_logs_flow() {
    if !can_bind_loopback().await {
        eprintln!("skipping e2e logs test: cannot bind to loopback in this environment");
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

    // 3. Send OTLP logs
    let otlp_payload = include_str!("fixtures/sample_otlp.json");
    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/logs", app_port))
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

    // Verify expected fields exist
    let first = &events[0];
    assert!(
        first.get("severity_text").is_some(),
        "missing severity_text"
    );
    assert!(first.get("body").is_some(), "missing body");
    assert!(
        first.get("resource_attributes").is_some(),
        "missing resource_attributes"
    );

    // 5. Cleanup
    mock_proc.stop().await;
}
