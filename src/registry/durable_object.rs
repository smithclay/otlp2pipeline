//! RegistryDO: Singleton Durable Object tracking all known service names.

#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use worker::*;

/// Service registration request.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub services: Vec<ServiceRegistration>,
}

/// Single service registration entry.
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Deserialize)]
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

        // Initialize schema on creation
        if let Err(e) = do_instance.ensure_schema() {
            panic!("Failed to initialize SQLite schema: {}", e);
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
    const DDL: &'static str = "CREATE TABLE IF NOT EXISTS services (
        name TEXT PRIMARY KEY,
        first_seen_at INTEGER NOT NULL,
        has_logs INTEGER DEFAULT 0,
        has_traces INTEGER DEFAULT 0,
        has_metrics INTEGER DEFAULT 0
    )";

    fn ensure_schema(&self) -> Result<()> {
        self.state.storage().sql().exec(Self::DDL, None)?;
        Ok(())
    }

    fn now_ms() -> i64 {
        worker::Date::now().as_millis() as i64
    }

    async fn handle_register(&self, mut req: Request) -> Result<Response> {
        let body = req.text().await?;
        let request: RegisterRequest = serde_json::from_str(&body)
            .map_err(|e| worker::Error::RustError(format!("Invalid JSON: {}", e)))?;

        if request.services.is_empty() {
            return Response::ok("0");
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
