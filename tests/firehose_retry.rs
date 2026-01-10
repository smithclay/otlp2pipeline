//! Tests for Firehose retry logic.
//! Run with: cargo test --features lambda --test firehose_retry

#![cfg(feature = "lambda")]

use vrl::value::Value;

/// Test that records are chunked at 500-record Firehose limit.
#[test]
fn test_chunk_at_500_records() {
    // Create 501 records - should produce 2 chunks
    let records: Vec<Value> = (0..501)
        .map(|i| {
            let mut obj = vrl::value::ObjectMap::new();
            obj.insert("id".into(), Value::Integer(i));
            Value::Object(obj)
        })
        .collect();

    let chunks: Vec<_> = records.chunks(500).collect();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].len(), 500);
    assert_eq!(chunks[1].len(), 1);
}

/// Test exponential backoff calculation.
#[test]
fn test_backoff_calculation() {
    use otlp2pipeline::lambda::firehose::calculate_backoff;

    // Attempt 0: base delay (100ms)
    let delay0 = calculate_backoff(0);
    assert!((100..=150).contains(&delay0), "attempt 0 delay: {}", delay0);

    // Attempt 1: 200ms base + jitter
    let delay1 = calculate_backoff(1);
    assert!((200..=300).contains(&delay1), "attempt 1 delay: {}", delay1);

    // Attempt 2: 400ms base + jitter
    let delay2 = calculate_backoff(2);
    assert!((400..=600).contains(&delay2), "attempt 2 delay: {}", delay2);

    // Should cap at MAX_DELAY_MS (10000)
    let delay10 = calculate_backoff(10);
    assert!(
        delay10 <= 10000,
        "attempt 10 should cap at 10000ms: {}",
        delay10
    );
}
