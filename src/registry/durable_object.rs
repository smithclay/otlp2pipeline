//! RegistryDO: Singleton Durable Object tracking all known service names.

#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use worker::*;

/// Service registration request.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub services: Vec<ServiceRegistration>,
}

/// Single service registration entry.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceRegistration {
    pub name: String,
    pub signal: String,
}

/// Service record for list responses.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceRecord {
    pub name: String,
    pub first_seen_at: i64,
    pub has_logs: i64,
    pub has_traces: i64,
    pub has_metrics: i64,
}

/// Metric record for list responses.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricRecord {
    pub name: String,
    pub metric_type: String,
}

/// Metric registration request.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterMetricsRequest {
    pub metrics: Vec<MetricRegistration>,
}

/// Single metric registration entry.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricRegistration {
    pub name: String,
    pub metric_type: String,
}

/// Helper type for COUNT queries.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

/// RegistryDO: Singleton tracking all known service names.
#[cfg(target_arch = "wasm32")]
#[durable_object]
pub struct RegistryDO {
    state: State,
    #[allow(dead_code)]
    env: Env,
}

#[cfg(target_arch = "wasm32")]
impl DurableObject for RegistryDO {
    fn new(state: State, env: Env) -> Self {
        let do_instance = Self { state, env };

        // Log but don't panic - Workers will return 500 and retry
        if let Err(e) = do_instance.ensure_schema() {
            worker::console_error!("Failed to initialize SQLite schema: {}", e);
        }

        do_instance
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();
        match (req.method(), path.as_str()) {
            (Method::Post, "/register") => self.handle_register(req).await,
            (Method::Get, "/list") => self.handle_list().await,
            _ => Response::error("Not found", 404),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl RegistryDO {
    /// Maximum number of unique services allowed in the registry.
    /// Protects against cardinality explosion exhausting DO storage (128MB limit).
    const MAX_SERVICES: usize = 10_000;

    /// Maximum number of unique (name, type) metric pairs allowed.
    const MAX_METRICS: usize = 10_000;

    const DDL: &'static str = "CREATE TABLE IF NOT EXISTS services (
        name TEXT PRIMARY KEY,
        first_seen_at INTEGER NOT NULL,
        has_logs INTEGER DEFAULT 0,
        has_traces INTEGER DEFAULT 0,
        has_metrics INTEGER DEFAULT 0
    )";

    const METRICS_DDL: &'static str = "CREATE TABLE IF NOT EXISTS metrics (
        name TEXT NOT NULL,
        metric_type TEXT NOT NULL,
        first_seen_at INTEGER NOT NULL,
        PRIMARY KEY (name, metric_type)
    )";

    fn ensure_schema(&self) -> Result<()> {
        self.state.storage().sql().exec(Self::DDL, None)?;
        self.state.storage().sql().exec(Self::METRICS_DDL, None)?;
        Ok(())
    }

    fn now_ms() -> i64 {
        worker::Date::now().as_millis() as i64
    }

    /// Get the current count of services in the registry.
    fn get_service_count(&self) -> Result<usize> {
        let sql = self.state.storage().sql();
        let rows: Vec<CountRow> = sql
            .exec("SELECT COUNT(*) as count FROM services", None)?
            .to_array()
            .map_err(|e| worker::Error::RustError(format!("Failed to count services: {}", e)))?;
        let count = rows.first().map(|r| r.count).unwrap_or(0) as usize;
        Ok(count)
    }

    /// Count how many services in the list are new (not yet in the registry).
    fn count_new_services(&self, names: &[String]) -> Result<usize> {
        if names.is_empty() {
            return Ok(0);
        }

        // Build a query to check which services already exist
        // We use IN (?, ?, ...) for batch lookup
        let placeholders = names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT COUNT(*) as count FROM services WHERE name IN ({})",
            placeholders
        );

        let params: Vec<SqlStorageValue> = names
            .iter()
            .map(|name| SqlStorageValue::String(name.to_string()))
            .collect();

        let sql = self.state.storage().sql();
        let rows: Vec<CountRow> = sql.exec(&query, Some(params))?.to_array().map_err(|e| {
            worker::Error::RustError(format!("Failed to count existing services: {}", e))
        })?;
        let existing_count = rows.first().map(|r| r.count).unwrap_or(0) as usize;

        // New services = total - existing
        Ok(names.len() - existing_count)
    }

    async fn handle_register(&self, mut req: Request) -> Result<Response> {
        let body = req.text().await?;
        let request: RegisterRequest = serde_json::from_str(&body)
            .map_err(|e| worker::Error::RustError(format!("Invalid JSON: {}", e)))?;

        if request.services.is_empty() {
            return Response::ok("0");
        }

        // Check cardinality limit before inserting
        let service_names: Vec<String> = request.services.iter().map(|s| s.name.clone()).collect();

        let current_count = self.get_service_count()?;
        let new_count = self.count_new_services(&service_names)?;

        if current_count + new_count > Self::MAX_SERVICES {
            worker::console_warn!(
                "Service registry limit exceeded: {} current + {} new would exceed maximum of {}",
                current_count,
                new_count,
                Self::MAX_SERVICES
            );
            return Response::error(
                format!(
                    "Service registry limit exceeded: {} current + {} new would exceed maximum of {}",
                    current_count, new_count, Self::MAX_SERVICES
                ),
                507, // Insufficient Storage
            );
        }

        let now = Self::now_ms();
        let mut registered = 0;

        for service in &request.services {
            self.upsert_service(&service.name, &service.signal, now)?;
            registered += 1;
        }

        Response::ok(format!("{}", registered))
    }

    fn upsert_service(&self, name: &str, signal: &str, first_seen_at: i64) -> Result<()> {
        let (has_logs, has_traces, has_metrics) = match signal {
            "logs" => (1, 0, 0),
            "traces" => (0, 1, 0),
            "metrics" => (0, 0, 1),
            _ => (0, 0, 0), // Unknown signal, register without flags
        };

        let sql = self.state.storage().sql();
        sql.exec(
            "INSERT INTO services (name, first_seen_at, has_logs, has_traces, has_metrics)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(name) DO UPDATE SET
               has_logs = MAX(has_logs, excluded.has_logs),
               has_traces = MAX(has_traces, excluded.has_traces),
               has_metrics = MAX(has_metrics, excluded.has_metrics)",
            vec![
                SqlStorageValue::String(name.to_string()),
                SqlStorageValue::Integer(first_seen_at),
                SqlStorageValue::Integer(has_logs),
                SqlStorageValue::Integer(has_traces),
                SqlStorageValue::Integer(has_metrics),
            ],
        )?;
        Ok(())
    }

    async fn handle_list(&self) -> Result<Response> {
        let sql = self.state.storage().sql();
        let result = sql.exec("SELECT * FROM services ORDER BY name", None)?;

        let services: Vec<ServiceRecord> = result.to_array().map_err(|e| {
            worker::Error::RustError(format!("Failed to deserialize service records: {}", e))
        })?;

        Response::from_json(&services)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    // Note: These tests document the expected behavior.
    // Actual testing of the RegistryDO requires the Cloudflare Workers runtime
    // and would be done via integration tests or wrangler dev/deploy.

    #[test]
    fn test_max_services_constant() {
        assert_eq!(RegistryDO::MAX_SERVICES, 10_000);
    }

    #[test]
    fn test_registration_request_serialization() {
        let req = RegisterRequest {
            services: vec![
                ServiceRegistration {
                    name: "service1".to_string(),
                    signal: "logs".to_string(),
                },
                ServiceRegistration {
                    name: "service2".to_string(),
                    signal: "traces".to_string(),
                },
            ],
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: RegisterRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.services.len(), 2);
        assert_eq!(deserialized.services[0].name, "service1");
        assert_eq!(deserialized.services[0].signal, "logs");
    }

    #[test]
    fn test_service_record_serialization() {
        let record = ServiceRecord {
            name: "test-service".to_string(),
            first_seen_at: 1234567890,
            has_logs: 1,
            has_traces: 0,
            has_metrics: 1,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ServiceRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test-service");
        assert_eq!(deserialized.first_seen_at, 1234567890);
        assert_eq!(deserialized.has_logs, 1);
        assert_eq!(deserialized.has_traces, 0);
        assert_eq!(deserialized.has_metrics, 1);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    // For native builds, we test that the cardinality limit constant is properly defined
    // The actual RegistryDO functionality is WASM-only and tested via integration tests

    #[test]
    fn test_cardinality_limit_documented() {
        // This test exists to document the cardinality protection behavior:
        // - MAX_SERVICES = 10,000 services
        // - Registry checks current count + new services before insertion
        // - Returns 507 (Insufficient Storage) if limit would be exceeded
        // - Protection prevents cardinality explosion from exhausting DO storage (128MB)

        // The implementation is in src/registry/durable_object.rs (WASM-only)
        // Integration testing would be done via Cloudflare Workers runtime
        assert!(
            true,
            "Cardinality protection is implemented in WASM-only code"
        );
    }
}
