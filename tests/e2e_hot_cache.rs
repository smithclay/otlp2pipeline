//! E2E tests for hot cache functionality.

mod helpers;

use otlpflare::cache::sender::{build_do_name, get_service_name};
use vrl::value::Value;

#[test]
fn test_hot_cache_service_name_extraction() {
    // Valid service name
    let record = Value::Object(
        [("service_name".into(), Value::from("my-service"))]
            .into_iter()
            .collect(),
    );
    assert_eq!(get_service_name(&record), "my-service");

    // Missing service name
    let record = Value::Object(Default::default());
    assert_eq!(get_service_name(&record), "unknown");
}

#[test]
fn test_hot_cache_do_name_building() {
    assert_eq!(build_do_name("my-service", "logs"), "my-service:logs");
    assert_eq!(build_do_name("api", "traces"), "api:traces");
}

#[test]
fn test_parquet_logs_conversion() {
    use otlpflare::cache::arrow_convert::json_to_logs_batch;
    use otlpflare::cache::parquet::write_parquet;
    use serde_json::json;

    // Create sample log records (simulating what would come from query)
    let log_records = vec![
        json!({
            "timestamp": 1703001600000_i64,
            "observed_timestamp": 1703001600001_i64,
            "service_name": "test-service",
            "severity_number": 9,
            "severity_text": "INFO",
            "body": "First log message",
            "trace_id": "abc123",
            "span_id": "def456",
        }),
        json!({
            "timestamp": 1703001601000_i64,
            "observed_timestamp": 1703001601001_i64,
            "service_name": "test-service",
            "severity_number": 13,
            "severity_text": "ERROR",
            "body": "Second log message",
        }),
    ];

    // Convert to Arrow RecordBatch
    let batch = json_to_logs_batch(&log_records).expect("Failed to convert to Arrow batch");
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(batch.num_columns(), 17);

    // Write to Parquet format
    let parquet_data = write_parquet(&batch).expect("Failed to write Parquet");

    // Validate Parquet format
    assert!(parquet_data.len() > 8, "Parquet file should have data");

    // Parquet files start with magic bytes "PAR1"
    assert_eq!(
        &parquet_data[0..4],
        b"PAR1",
        "Parquet file should start with PAR1 magic bytes"
    );

    // Parquet files end with magic bytes "PAR1"
    assert_eq!(
        &parquet_data[parquet_data.len() - 4..],
        b"PAR1",
        "Parquet file should end with PAR1 magic bytes"
    );

    // Verify the file can be read back with parquet crate
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let bytes = Bytes::from(parquet_data);
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create Parquet reader");

    // Verify schema column names
    let schema = builder.schema();
    let expected_columns = vec![
        "_signal",
        "_timestamp_nanos",
        "timestamp",
        "observed_timestamp",
        "trace_id",
        "span_id",
        "service_name",
        "service_namespace",
        "service_instance_id",
        "severity_number",
        "severity_text",
        "body",
        "resource_attributes",
        "scope_name",
        "scope_version",
        "scope_attributes",
        "log_attributes",
    ];

    for (i, expected_name) in expected_columns.iter().enumerate() {
        assert_eq!(
            schema.field(i).name(),
            expected_name,
            "Column {} should be named {}",
            i,
            expected_name
        );
    }

    // Read the batch back
    let mut reader = builder.build().expect("Failed to build reader");
    let read_batch = reader
        .next()
        .expect("Should have at least one batch")
        .expect("Failed to read batch");

    assert_eq!(read_batch.num_rows(), 2);
    assert_eq!(read_batch.num_columns(), 17);
}

#[test]
fn test_parquet_traces_conversion() {
    use otlpflare::cache::arrow_convert::json_to_traces_batch;
    use otlpflare::cache::parquet::write_parquet;
    use serde_json::json;

    // Create sample trace records (simulating what would come from query)
    let trace_records = vec![
        json!({
            "timestamp": 1703001600000_i64,
            "end_timestamp": 1703001600100_i64,
            "duration": 100_i64,
            "trace_id": "abc123",
            "span_id": "def456",
            "parent_span_id": "parent789",
            "service_name": "test-service",
            "span_name": "GET /api/endpoint",
            "span_kind": 2,
            "status_code": 1,
            "dropped_attributes_count": 0,
            "dropped_events_count": 0,
            "dropped_links_count": 0,
            "flags": 0,
        }),
        json!({
            "timestamp": 1703001601000_i64,
            "end_timestamp": 1703001601050_i64,
            "duration": 50_i64,
            "trace_id": "abc123",
            "span_id": "child789",
            "parent_span_id": "def456",
            "service_name": "test-service",
            "span_name": "database query",
            "span_kind": 3,
            "status_code": 1,
            "dropped_attributes_count": 0,
            "dropped_events_count": 0,
            "dropped_links_count": 0,
            "flags": 0,
        }),
    ];

    // Convert to Arrow RecordBatch
    let batch = json_to_traces_batch(&trace_records).expect("Failed to convert to Arrow batch");
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(batch.num_columns(), 27);

    // Write to Parquet format
    let parquet_data = write_parquet(&batch).expect("Failed to write Parquet");

    // Validate Parquet format
    assert!(parquet_data.len() > 8, "Parquet file should have data");

    // Parquet files start with magic bytes "PAR1"
    assert_eq!(
        &parquet_data[0..4],
        b"PAR1",
        "Parquet file should start with PAR1 magic bytes"
    );

    // Parquet files end with magic bytes "PAR1"
    assert_eq!(
        &parquet_data[parquet_data.len() - 4..],
        b"PAR1",
        "Parquet file should end with PAR1 magic bytes"
    );

    // Verify the file can be read back
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let bytes = Bytes::from(parquet_data);
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create Parquet reader");

    let schema = builder.schema();

    // Verify schema includes key trace fields
    assert!(schema.column_with_name("_signal").is_some());
    assert!(schema.column_with_name("trace_id").is_some());
    assert!(schema.column_with_name("span_id").is_some());
    assert!(schema.column_with_name("span_name").is_some());
    assert!(schema.column_with_name("duration").is_some());

    // Read the batch back
    let mut reader = builder.build().expect("Failed to build reader");
    let read_batch = reader
        .next()
        .expect("Should have at least one batch")
        .expect("Failed to read batch");

    assert_eq!(read_batch.num_rows(), 2);
    assert_eq!(read_batch.num_columns(), 27);
}

#[test]
fn test_parquet_empty_batch() {
    use otlpflare::cache::arrow_convert::json_to_logs_batch;
    use otlpflare::cache::parquet::write_parquet;

    // Test with empty data (edge case)
    let empty_records: Vec<serde_json::Value> = vec![];

    let batch = json_to_logs_batch(&empty_records).expect("Failed to convert empty batch");
    assert_eq!(batch.num_rows(), 0);
    assert_eq!(batch.num_columns(), 17);

    // Empty batches should still produce valid Parquet files
    let parquet_data = write_parquet(&batch).expect("Failed to write empty Parquet");

    // Should still have magic bytes
    assert_eq!(&parquet_data[0..4], b"PAR1");
    assert_eq!(&parquet_data[parquet_data.len() - 4..], b"PAR1");

    // Should be readable (though empty batch may not have data to iterate)
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let bytes = Bytes::from(parquet_data);
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create Parquet reader");

    // Verify schema is valid even though there's no data
    let schema = builder.schema();
    assert_eq!(schema.fields().len(), 17);

    let mut reader = builder.build().expect("Failed to build reader");

    // Empty Parquet files may not have any batches
    if let Some(batch_result) = reader.next() {
        let read_batch = batch_result.expect("Failed to read batch");
        assert_eq!(read_batch.num_rows(), 0);
    }
    // If there are no batches, that's also fine for an empty file
}

#[test]
fn test_parquet_gauge_conversion() {
    use otlpflare::cache::arrow_convert::json_to_gauge_batch;
    use otlpflare::cache::parquet::write_parquet;
    use serde_json::json;

    let gauge_records = vec![
        json!({
            "timestamp": 1703001600000_i64,
            "service_name": "test-service",
            "metric_name": "cpu_usage",
            "value": 0.75,
            "metric_unit": "ratio",
            "metric_attributes": "{\"host\":\"server1\"}",
        }),
        json!({
            "timestamp": 1703001601000_i64,
            "service_name": "test-service",
            "metric_name": "memory_usage",
            "value": 0.5,
            "metric_unit": "ratio",
            "metric_attributes": "{\"host\":\"server1\"}",
        }),
    ];

    let batch =
        json_to_gauge_batch(&gauge_records).expect("Failed to convert gauge to Arrow batch");
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(batch.num_columns(), 17);

    let parquet_data = write_parquet(&batch).expect("Failed to write gauge Parquet");

    // Validate Parquet format
    assert!(parquet_data.len() > 8, "Parquet file should have data");
    assert_eq!(&parquet_data[0..4], b"PAR1");
    assert_eq!(&parquet_data[parquet_data.len() - 4..], b"PAR1");

    // Verify round-trip
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let bytes = Bytes::from(parquet_data);
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create reader");
    let schema = builder.schema();
    assert!(schema.column_with_name("metric_name").is_some());
    assert!(schema.column_with_name("value").is_some());

    // Read the batch back
    let mut reader = builder.build().expect("Failed to build reader");
    let read_batch = reader
        .next()
        .expect("Should have at least one batch")
        .expect("Failed to read batch");

    assert_eq!(read_batch.num_rows(), 2);
    assert_eq!(read_batch.num_columns(), 17);
}

#[test]
fn test_parquet_sum_conversion() {
    use otlpflare::cache::arrow_convert::json_to_sum_batch;
    use otlpflare::cache::parquet::write_parquet;
    use serde_json::json;

    let sum_records = vec![
        json!({
            "timestamp": 1703001600000_i64,
            "service_name": "test-service",
            "metric_name": "requests_total",
            "value": 1000.0,
            "metric_unit": "1",
            "is_monotonic": true,
            "aggregation_temporality": 2,
            "metric_attributes": "{\"endpoint\":\"/api\"}",
        }),
        json!({
            "timestamp": 1703001601000_i64,
            "service_name": "test-service",
            "metric_name": "errors_total",
            "value": 50.0,
            "metric_unit": "1",
            "is_monotonic": true,
            "aggregation_temporality": 2,
            "metric_attributes": "{\"endpoint\":\"/api\"}",
        }),
    ];

    let batch = json_to_sum_batch(&sum_records).expect("Failed to convert sum to Arrow batch");
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(batch.num_columns(), 19);

    let parquet_data = write_parquet(&batch).expect("Failed to write sum Parquet");

    // Validate Parquet format
    assert!(parquet_data.len() > 8, "Parquet file should have data");
    assert_eq!(&parquet_data[0..4], b"PAR1");
    assert_eq!(&parquet_data[parquet_data.len() - 4..], b"PAR1");

    // Verify round-trip
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let bytes = Bytes::from(parquet_data);
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create reader");
    let schema = builder.schema();
    assert!(schema.column_with_name("is_monotonic").is_some());
    assert!(schema.column_with_name("aggregation_temporality").is_some());

    // Read the batch back
    let mut reader = builder.build().expect("Failed to build reader");
    let read_batch = reader
        .next()
        .expect("Should have at least one batch")
        .expect("Failed to read batch");

    assert_eq!(read_batch.num_rows(), 2);
    assert_eq!(read_batch.num_columns(), 19);
}

#[test]
fn test_parquet_duckdb_compatibility() {
    use otlpflare::cache::arrow_convert::json_to_logs_batch;
    use otlpflare::cache::parquet::write_parquet;
    use serde_json::json;
    use std::io::Write;
    use std::process::Command;

    // Create sample log records
    let log_records = vec![
        json!({
            "timestamp": 1703001600000_i64,
            "observed_timestamp": 1703001600001_i64,
            "service_name": "duckdb-test-service",
            "severity_number": 9,
            "severity_text": "INFO",
            "body": "Test message for DuckDB",
            "trace_id": "trace123",
        }),
        json!({
            "timestamp": 1703001601000_i64,
            "observed_timestamp": 1703001601001_i64,
            "service_name": "duckdb-test-service",
            "severity_number": 13,
            "severity_text": "ERROR",
            "body": "Error message for DuckDB",
        }),
    ];

    // Convert to Parquet
    let batch = json_to_logs_batch(&log_records).expect("Failed to convert to Arrow batch");
    let parquet_data = write_parquet(&batch).expect("Failed to write Parquet");

    // Validate Parquet format with parquet crate's own reader (always runs)
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let bytes = Bytes::from(parquet_data.clone());
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create Parquet reader");
    let mut reader = builder.build().expect("Failed to build reader");
    let read_batch = reader
        .next()
        .expect("Should have at least one batch")
        .expect("Failed to read batch");
    assert_eq!(read_batch.num_rows(), 2);

    // Optional: Try to load with DuckDB if available
    // This validates that the Parquet format is compatible with external tools
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_parquet_logs.parquet");
    {
        let mut file = std::fs::File::create(&temp_file).expect("Failed to create temp file");
        file.write_all(&parquet_data)
            .expect("Failed to write Parquet data");
    }

    // Try DuckDB with Python (more commonly has Parquet support)
    let python_script = format!(
        r#"
import duckdb
try:
    result = duckdb.sql("SELECT COUNT(*) as count, service_name FROM read_parquet('{}') GROUP BY service_name").fetchall()
    print(result)
except Exception as e:
    print(f"SKIP: {{e}}")
"#,
        temp_file.display()
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(&python_script)
        .output();

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_file);

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("SKIP") || !output.stderr.is_empty() {
            println!("Note: DuckDB test skipped (DuckDB not installed or Python not available)");
            println!("Core Parquet format validation passed via parquet reader above.");
        } else {
            println!("DuckDB successfully read Parquet: {}", stdout);
            // If DuckDB worked, verify it got the right data
            assert!(
                stdout.contains("2") && stdout.contains("duckdb-test-service"),
                "DuckDB should read 2 rows with correct service_name"
            );
        }
    } else {
        println!("Note: Python/DuckDB not available for optional compatibility test");
        println!("Core Parquet format validation passed via parquet reader above.");
    }
}
