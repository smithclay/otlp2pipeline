// src/aggregator/stats.rs
//! Aggregation types for logs and traces.

use serde_json::Value;

/// OpenTelemetry severity numbers: values 17-24 represent error-level events
/// https://opentelemetry.io/docs/specs/otel/logs/data-model/
const SEVERITY_ERROR_THRESHOLD: i64 = 17;

/// OpenTelemetry span status codes: 0=Unset, 1=Ok, 2=Error
/// https://opentelemetry.io/docs/specs/otel/trace/api/#set-status
const STATUS_CODE_ERROR: i64 = 2;

/// Log aggregates: count and error count (severity >= 17).
#[derive(Default, Debug)]
pub struct LogAggregates {
    pub count: i64,
    pub error_count: i64,
}

impl LogAggregates {
    pub fn accumulate(&mut self, record: &Value) {
        self.count += 1;
        if let Some(severity) = record.get("severity_number").and_then(|v| v.as_i64()) {
            if severity >= SEVERITY_ERROR_THRESHOLD {
                self.error_count += 1;
            }
        }
    }
}

/// Trace aggregates: count, error count, and latency stats.
#[derive(Default, Debug)]
pub struct TraceAggregates {
    pub count: i64,
    pub error_count: i64,
    pub latency_sum_us: i64,
    pub latency_min_us: Option<i64>,
    pub latency_max_us: Option<i64>,
}

impl TraceAggregates {
    pub fn accumulate(&mut self, record: &Value) {
        self.count += 1;

        // Error: status_code == 2
        if let Some(status) = record.get("status_code").and_then(|v| v.as_i64()) {
            if status == STATUS_CODE_ERROR {
                self.error_count += 1;
            }
        }

        // Latency: VRL outputs "duration" in milliseconds, convert to microseconds
        if let Some(duration_ms) = record.get("duration").and_then(|v| v.as_i64()) {
            let duration_us = duration_ms * 1000;
            self.latency_sum_us += duration_us;
            self.latency_min_us = Some(
                self.latency_min_us
                    .map(|min| min.min(duration_us))
                    .unwrap_or(duration_us),
            );
            self.latency_max_us = Some(
                self.latency_max_us
                    .map(|max| max.max(duration_us))
                    .unwrap_or(duration_us),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn log_aggregates_counts_records() {
        let mut agg = LogAggregates::default();
        agg.accumulate(&json!({"severity_number": 9}));
        agg.accumulate(&json!({"severity_number": 17}));
        agg.accumulate(&json!({"severity_number": 21}));

        assert_eq!(agg.count, 3);
        assert_eq!(agg.error_count, 2); // 17 and 21 are errors
    }

    #[test]
    fn trace_aggregates_tracks_latency() {
        let mut agg = TraceAggregates::default();
        // VRL outputs "duration" in milliseconds (after converting from duration_ns)
        agg.accumulate(&json!({"status_code": 0, "duration": 1})); // 1ms
        agg.accumulate(&json!({"status_code": 2, "duration": 5})); // 5ms, error
        agg.accumulate(&json!({"status_code": 1, "duration": 2})); // 2ms

        assert_eq!(agg.count, 3);
        assert_eq!(agg.error_count, 1);
        assert_eq!(agg.latency_sum_us, 8000); // 1000 + 5000 + 2000 microseconds
        assert_eq!(agg.latency_min_us, Some(1000)); // 1ms = 1000μs
        assert_eq!(agg.latency_max_us, Some(5000)); // 5ms = 5000μs
    }

    #[test]
    fn log_aggregates_handles_missing_severity() {
        let mut agg = LogAggregates::default();
        agg.accumulate(&json!({"body": "test"})); // no severity_number
        assert_eq!(agg.count, 1);
        assert_eq!(agg.error_count, 0);
    }

    #[test]
    fn log_aggregates_severity_boundary() {
        let mut agg = LogAggregates::default();
        agg.accumulate(&json!({"severity_number": 16})); // WARN (not error)
        agg.accumulate(&json!({"severity_number": 17})); // ERROR
        assert_eq!(agg.error_count, 1);
    }

    #[test]
    fn trace_aggregates_handles_missing_duration() {
        let mut agg = TraceAggregates::default();
        agg.accumulate(&json!({"status_code": 0})); // no duration field
        assert_eq!(agg.count, 1);
        assert_eq!(agg.latency_sum_us, 0);
        assert_eq!(agg.latency_min_us, None);
    }

    #[test]
    fn trace_aggregates_matches_vrl_output_format() {
        // This test verifies the aggregator works with VRL's actual output format:
        // - VRL outputs "duration" in milliseconds (not duration_ns)
        // - VRL outputs "status_code" as integer
        // - Aggregator converts ms -> μs for storage
        let mut agg = TraceAggregates::default();

        // Simulate VRL output: span with 100ms duration (like sample_otlp_traces.json)
        let vrl_output = json!({
            "trace_id": "0af7651916cd43dd8448eb211c80319c",
            "span_id": "b7ad6b7169203331",
            "span_name": "HTTP GET /api/users",
            "duration": 100,      // VRL outputs milliseconds
            "status_code": 1,     // OK
            "service_name": "my-service"
        });

        agg.accumulate(&vrl_output);

        assert_eq!(agg.count, 1);
        assert_eq!(agg.error_count, 0);
        assert_eq!(agg.latency_sum_us, 100_000); // 100ms = 100,000μs
        assert_eq!(agg.latency_min_us, Some(100_000));
        assert_eq!(agg.latency_max_us, Some(100_000));
    }

    #[test]
    fn trace_aggregates_error_spans() {
        let mut agg = TraceAggregates::default();

        // status_code=2 means error in OTLP
        agg.accumulate(&json!({"status_code": 2, "duration": 50}));
        agg.accumulate(&json!({"status_code": 1, "duration": 30}));
        agg.accumulate(&json!({"status_code": 2, "duration": 70}));

        assert_eq!(agg.count, 3);
        assert_eq!(agg.error_count, 2); // Two spans with status_code=2
        assert_eq!(agg.latency_sum_us, 150_000); // (50+30+70)ms = 150,000μs
        assert_eq!(agg.latency_min_us, Some(30_000));
        assert_eq!(agg.latency_max_us, Some(70_000));
    }
}
