//! Arrow RecordBatch to Parquet conversion.

use arrow_array::RecordBatch;
use parquet::arrow::ArrowWriter;
use std::io::Cursor;

/// Error type for Parquet conversion.
#[derive(Debug)]
pub struct ParquetConvertError(pub String);

impl std::fmt::Display for ParquetConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parquet conversion error: {}", self.0)
    }
}

impl std::error::Error for ParquetConvertError {}

/// Write a RecordBatch to Parquet format.
///
/// Returns the raw Parquet bytes suitable for HTTP response body.
/// No compression codec is used - HTTP transport compression (gzip/brotli) handles it.
pub fn write_parquet(batch: &RecordBatch) -> Result<Vec<u8>, ParquetConvertError> {
    let mut buffer = Cursor::new(Vec::new());
    let mut writer = ArrowWriter::try_new(&mut buffer, batch.schema(), None)
        .map_err(|e| ParquetConvertError(e.to_string()))?;
    writer
        .write(batch)
        .map_err(|e| ParquetConvertError(e.to_string()))?;
    writer
        .close()
        .map_err(|e| ParquetConvertError(e.to_string()))?;
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::arrow_convert::json_to_logs_batch;
    use serde_json::json;

    #[test]
    fn test_write_parquet_empty_batch() {
        let batch = json_to_logs_batch(&[]).unwrap();
        let parquet_bytes = write_parquet(&batch).unwrap();

        // Parquet files start with magic bytes "PAR1"
        assert!(parquet_bytes.len() >= 4);
        assert_eq!(&parquet_bytes[0..4], b"PAR1");
    }

    #[test]
    fn test_write_parquet_with_data() {
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
        let parquet_bytes = write_parquet(&batch).unwrap();

        // Parquet files start with "PAR1" and end with "PAR1"
        assert!(parquet_bytes.len() > 8);
        assert_eq!(&parquet_bytes[0..4], b"PAR1");
        assert_eq!(&parquet_bytes[parquet_bytes.len() - 4..], b"PAR1");
    }
}
