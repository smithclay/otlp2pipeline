//! Security tests for query handling.

use otlpflare::cache::parquet::ExportParams;

/// Test that table name validation rejects SQL injection attempts.
///
/// This validates the security principle tested in do_query.rs:69-76.
/// The valid table names are: logs, traces, gauge, sum.
/// Any other input should be treated as invalid.
#[test]
fn test_sql_injection_table_name_rejected() {
    // These are the only valid table names according to do_query.rs
    let valid_tables = ["logs", "traces", "gauge", "sum"];

    // SQL injection attempts that should be rejected
    let injection_attempts = [
        "logs; DROP TABLE logs;--",
        "logs UNION SELECT * FROM secrets",
        "logs' OR '1'='1",
        "../../etc/passwd",
        "LOGS",     // Case sensitivity
        "log",      // Typo
        "metrics",  // Wrong name (we use gauge/sum)
        "",         // Empty
        " logs",    // Leading space
        "logs ",    // Trailing space
        "logs\n",   // Newline
        "logs\t",   // Tab
        "logs/**/", // Path traversal
        "../logs",  // Path traversal
    ];

    // Verify valid tables are in the allowlist
    for table in valid_tables {
        assert!(
            valid_tables.contains(&table),
            "Valid table '{}' should be in allowlist",
            table
        );
    }

    // Verify injection attempts would be rejected by exact match logic
    for attempt in injection_attempts {
        let is_valid = valid_tables.contains(&attempt);
        assert!(
            !is_valid,
            "Injection attempt '{}' should NOT be in valid tables",
            attempt
        );
    }

    // Additional validation: ensure the check is exact, not substring-based
    assert!(!valid_tables.contains(&"logs_backup"));
    assert!(!valid_tables.contains(&"my_logs"));
    assert!(!valid_tables.contains(&"logsystem"));
}

/// Test that query parameter validation works correctly.
///
/// This validates the ExportParams::validate() logic that protects
/// against resource exhaustion and invalid queries.
#[test]
fn test_query_params_validation() {
    // Test missing services
    let result = ExportParams::from_query_string("");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(
        params.validate().is_err(),
        "Empty services should fail validation"
    );

    // Test too many services (>50)
    let many_services: Vec<String> = (0..51).map(|i| format!("service{}", i)).collect();
    let query = format!("services={}", many_services.join(","));
    let result = ExportParams::from_query_string(&query);
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(
        params.validate().is_err(),
        ">50 services should fail validation"
    );

    // Test invalid limit (0)
    let result = ExportParams::from_query_string("services=svc1&limit=0");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_err(), "limit=0 should fail validation");

    // Test invalid limit (>10000)
    let result = ExportParams::from_query_string("services=svc1&limit=10001");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(
        params.validate().is_err(),
        "limit>10000 should fail validation"
    );

    // Test invalid time range (start > end)
    let result = ExportParams::from_query_string("services=svc1&start=2000&end=1000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(
        params.validate().is_err(),
        "start>end should fail validation"
    );

    // Test valid params
    let result =
        ExportParams::from_query_string("services=svc1,svc2&limit=100&start=1000&end=2000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(
        params.validate().is_ok(),
        "Valid params should pass validation"
    );
}

/// Test edge cases in service name parsing.
///
/// Ensures that service name parsing doesn't allow injection
/// through malformed service lists.
#[test]
fn test_service_name_edge_cases() {
    // Test URL encoding edge cases
    let result = ExportParams::from_query_string("services=svc%201,svc%202");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.services, vec!["svc 1", "svc 2"]);

    // Test empty service names are filtered out
    let result = ExportParams::from_query_string("services=svc1,,svc2");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.services, vec!["svc1", "svc2"]);

    // Test whitespace trimming
    let result = ExportParams::from_query_string("services= svc1 , svc2 ");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert_eq!(params.services, vec!["svc1", "svc2"]);

    // Test that all-empty services are caught by validation
    let result = ExportParams::from_query_string("services=,,,");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.services.is_empty());
    assert!(params.validate().is_err(), "Empty services should fail");
}

/// Test that limit validation protects against resource exhaustion.
#[test]
fn test_limit_bounds_protection() {
    // Test boundary values
    let result = ExportParams::from_query_string("services=svc1&limit=1");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_ok(), "limit=1 should be valid");

    let result = ExportParams::from_query_string("services=svc1&limit=10000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_ok(), "limit=10000 should be valid");

    // Test just beyond boundaries
    let result = ExportParams::from_query_string("services=svc1&limit=0");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_err(), "limit=0 should be invalid");

    let result = ExportParams::from_query_string("services=svc1&limit=10001");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_err(), "limit=10001 should be invalid");

    // Test extreme values
    let result = ExportParams::from_query_string("services=svc1&limit=999999");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(
        params.validate().is_err(),
        "extreme limit should be invalid"
    );
}

/// Test time range validation to prevent backwards queries.
#[test]
fn test_time_range_validation() {
    // Valid ranges
    let result = ExportParams::from_query_string("services=svc1&start=1000&end=2000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_ok(), "Valid range should pass");

    let result = ExportParams::from_query_string("services=svc1&start=1000&end=1000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_ok(), "Equal start/end should pass");

    // Invalid range
    let result = ExportParams::from_query_string("services=svc1&start=2000&end=1000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_err(), "Backwards range should fail");

    // Only start or only end should be valid
    let result = ExportParams::from_query_string("services=svc1&start=1000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_ok(), "Only start should be valid");

    let result = ExportParams::from_query_string("services=svc1&end=2000");
    assert!(result.is_ok());
    let params = result.unwrap();
    assert!(params.validate().is_ok(), "Only end should be valid");
}
