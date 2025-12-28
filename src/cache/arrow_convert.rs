//! JSON to Arrow RecordBatch conversion.
//!
//! Converts serde_json::Value arrays to Arrow RecordBatches.

// Use generated schemas
include!(concat!(env!("OUT_DIR"), "/arrow_schemas.rs"));

/// Error type for Arrow conversion.
#[derive(Debug)]
pub struct ArrowConvertError(pub String);

impl std::fmt::Display for ArrowConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Arrow conversion error: {}", self.0)
    }
}

impl std::error::Error for ArrowConvertError {}

// Include generated Arrow converters
include!(concat!(env!("OUT_DIR"), "/arrow_convert_gen.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_empty_logs_batch() {
        let batch = json_to_logs_batch(&[]).unwrap();
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(batch.num_columns(), 17);
    }

    #[test]
    fn test_logs_batch_with_data() {
        let rows = vec![json!({
            "_signal": "logs",
            "_timestamp_nanos": 1704067200000000000_i64,
            "timestamp": 1704067200000_i64,
            "observed_timestamp": 1704067200001_i64,
            "service_name": "test-service",
            "severity_number": 9,
            "severity_text": "INFO",
            "body": "Test log message"
        })];
        let batch = json_to_logs_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 17);
    }

    #[test]
    fn test_empty_traces_batch() {
        let batch = json_to_traces_batch(&[]).unwrap();
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(batch.num_columns(), 27);
    }

    #[test]
    fn test_traces_batch_with_data() {
        let rows = vec![json!({
            "_signal": "spans",
            "_timestamp_nanos": 1704067200000000000_i64,
            "timestamp": 1704067200000_i64,
            "end_timestamp": 1704067200100_i64,
            "duration": 100_i64,
            "trace_id": "abc123",
            "span_id": "def456",
            "service_name": "test-service",
            "span_name": "test-span",
            "span_kind": 2,
            "status_code": 1,
            "dropped_attributes_count": 0,
            "dropped_events_count": 0,
            "dropped_links_count": 0,
            "flags": 0
        })];
        let batch = json_to_traces_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 27);
    }
}
