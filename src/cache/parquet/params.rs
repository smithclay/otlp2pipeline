//! Query parameter parsing and validation for parquet export endpoints.

use std::collections::BTreeSet;

/// Validation limits
pub const MAX_SERVICES: usize = 50;
pub const MAX_LIMIT: usize = 10_000;
pub const DEFAULT_LIMIT: usize = 1_000;

/// Export request parameters.
#[derive(Debug, Clone)]
pub struct ExportParams {
    /// Service names to query (required, max 50)
    pub services: Vec<String>,
    /// Start time in Unix seconds (optional)
    pub start: Option<f64>,
    /// End time in Unix seconds (optional)
    pub end: Option<f64>,
    /// Maximum rows to return (default 1000, max 10000)
    pub limit: usize,
    /// Filter by trace ID (logs/traces only)
    pub trace_id: Option<String>,
    /// Filter by metric name (metrics only)
    pub metric_name: Option<String>,
    /// Label filters as key=value pairs (metrics only)
    pub labels: Vec<(String, String)>,
}

/// Error type for parameter parsing/validation.
#[derive(Debug, Clone)]
pub struct ExportError {
    pub message: String,
    pub status_code: u16,
}

impl ExportError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            status_code: 400,
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            status_code: 404,
        }
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExportError {}

impl ExportParams {
    /// Parse parameters from URL query string.
    pub fn from_query_string(query: &str) -> Result<Self, ExportError> {
        let mut services = Vec::new();
        let mut start = None;
        let mut end = None;
        let mut limit = DEFAULT_LIMIT;
        let mut trace_id = None;
        let mut metric_name = None;
        let mut labels = Vec::new();

        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("");
            let value = urlencoding::decode(value).map_err(|_| {
                ExportError::bad_request(format!("Invalid URL encoding in {}", key))
            })?;

            match key {
                "services" => {
                    services = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "start" => {
                    start = Some(
                        value
                            .parse::<f64>()
                            .map_err(|_| ExportError::bad_request("Invalid start time"))?,
                    );
                }
                "end" => {
                    end = Some(
                        value
                            .parse::<f64>()
                            .map_err(|_| ExportError::bad_request("Invalid end time"))?,
                    );
                }
                "limit" => {
                    limit = value
                        .parse::<usize>()
                        .map_err(|_| ExportError::bad_request("Invalid limit"))?;
                }
                "trace_id" => {
                    trace_id = Some(value.to_string());
                }
                "metric_name" => {
                    metric_name = Some(value.to_string());
                }
                "labels" => {
                    // Parse key=value,key2=value2 format
                    for label_pair in value.split(',') {
                        let mut label_parts = label_pair.splitn(2, '=');
                        let label_key = label_parts.next().unwrap_or("").trim();
                        let label_value = label_parts.next().unwrap_or("").trim();
                        if !label_key.is_empty() {
                            labels.push((label_key.to_string(), label_value.to_string()));
                        }
                    }
                }
                _ => {} // Ignore unknown parameters
            }
        }

        Ok(Self {
            services,
            start,
            end,
            limit,
            trace_id,
            metric_name,
            labels,
        })
    }

    /// Validate parameters, returning error if invalid.
    pub fn validate(&self) -> Result<(), ExportError> {
        if self.services.is_empty() {
            return Err(ExportError::bad_request("services parameter is required"));
        }
        if self.services.len() > MAX_SERVICES {
            return Err(ExportError::bad_request(format!(
                "Too many services (max {})",
                MAX_SERVICES
            )));
        }
        if self.limit == 0 || self.limit > MAX_LIMIT {
            return Err(ExportError::bad_request(format!(
                "limit must be between 1 and {}",
                MAX_LIMIT
            )));
        }
        if let (Some(start), Some(end)) = (self.start, self.end) {
            if start > end {
                return Err(ExportError::bad_request("start must be <= end"));
            }
        }
        Ok(())
    }

    /// Generate DO names for the given signal type.
    /// Format: "{service}:{signal}"
    pub fn do_names(&self, signal: &str) -> Vec<String> {
        // Deduplicate services (BTreeSet ensures deterministic ordering)
        let unique: BTreeSet<_> = self.services.iter().collect();
        unique
            .into_iter()
            .map(|svc| format!("{}:{}", svc, signal))
            .collect()
    }

    /// Convert start time to milliseconds for SQLite query.
    pub fn start_ms(&self) -> Option<i64> {
        self.start.map(|s| (s * 1000.0) as i64)
    }

    /// Convert end time to milliseconds for SQLite query.
    pub fn end_ms(&self) -> Option<i64> {
        self.end.map(|e| (e * 1000.0) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_params() {
        let params = ExportParams::from_query_string("services=svc1,svc2&limit=100").unwrap();
        assert_eq!(params.services, vec!["svc1", "svc2"]);
        assert_eq!(params.limit, 100);
    }

    #[test]
    fn test_parse_time_range() {
        let params =
            ExportParams::from_query_string("services=svc1&start=1703721600&end=1703808000")
                .unwrap();
        assert_eq!(params.start, Some(1703721600.0));
        assert_eq!(params.end, Some(1703808000.0));
        assert_eq!(params.start_ms(), Some(1703721600000));
    }

    #[test]
    fn test_parse_trace_id() {
        let params = ExportParams::from_query_string("services=svc1&trace_id=abc123").unwrap();
        assert_eq!(params.trace_id, Some("abc123".to_string()));
    }

    #[test]
    fn test_parse_metric_params() {
        let params = ExportParams::from_query_string(
            "services=svc1&metric_name=cpu_usage&labels=host=h1,env=prod",
        )
        .unwrap();
        assert_eq!(params.metric_name, Some("cpu_usage".to_string()));
        assert_eq!(
            params.labels,
            vec![
                ("host".to_string(), "h1".to_string()),
                ("env".to_string(), "prod".to_string()),
            ]
        );
    }

    #[test]
    fn test_validate_missing_services() {
        let params = ExportParams::from_query_string("limit=100").unwrap();
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_validate_too_many_services() {
        let services: Vec<_> = (0..51).map(|i| format!("svc{}", i)).collect();
        let query = format!("services={}", services.join(","));
        let params = ExportParams::from_query_string(&query).unwrap();
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_validate_limit_bounds() {
        let params = ExportParams::from_query_string("services=svc1&limit=0").unwrap();
        assert!(params.validate().is_err());

        let params = ExportParams::from_query_string("services=svc1&limit=10001").unwrap();
        assert!(params.validate().is_err());

        let params = ExportParams::from_query_string("services=svc1&limit=10000").unwrap();
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_validate_time_range() {
        let params = ExportParams::from_query_string("services=svc1&start=1000&end=500").unwrap();
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_do_names() {
        let params = ExportParams::from_query_string("services=svc1,svc2,svc1").unwrap();
        let names = params.do_names("logs");
        assert_eq!(names.len(), 2); // Deduplicated
        assert!(names.contains(&"svc1:logs".to_string()));
        assert!(names.contains(&"svc2:logs".to_string()));
    }

    #[test]
    fn test_default_limit() {
        let params = ExportParams::from_query_string("services=svc1").unwrap();
        assert_eq!(params.limit, DEFAULT_LIMIT);
    }

    #[test]
    fn test_do_names_ordering_deterministic() {
        // Test that do_names produces deterministic ordering even with duplicates and random order
        let params = ExportParams::from_query_string("services=c,b,a,b,c").unwrap();
        let names1 = params.do_names("logs");
        let names2 = params.do_names("logs");
        assert_eq!(names1, names2); // Must be identical

        // Verify ordering is alphabetical (BTreeSet property)
        assert_eq!(names1, vec!["a:logs", "b:logs", "c:logs"]);
    }
}
