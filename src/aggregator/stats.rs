// src/aggregator/stats.rs
//! Aggregation types for logs and traces.

use serde_json::Value;

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
            if severity >= 17 {
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
            if status == 2 {
                self.error_count += 1;
            }
        }

        // Latency (duration_ns -> microseconds)
        if let Some(duration_ns) = record.get("duration_ns").and_then(|v| v.as_i64()) {
            let duration_us = duration_ns / 1000;
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
        agg.accumulate(&json!({"status_code": 0, "duration_ns": 1_000_000})); // 1ms
        agg.accumulate(&json!({"status_code": 2, "duration_ns": 5_000_000})); // 5ms, error
        agg.accumulate(&json!({"status_code": 1, "duration_ns": 2_000_000})); // 2ms

        assert_eq!(agg.count, 3);
        assert_eq!(agg.error_count, 1);
        assert_eq!(agg.latency_sum_us, 8000); // 1000 + 5000 + 2000
        assert_eq!(agg.latency_min_us, Some(1000));
        assert_eq!(agg.latency_max_us, Some(5000));
    }
}
