use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::CloudflareClient;

// ============ Streams ============

#[derive(Serialize)]
struct CreateStreamRequest<'a> {
    name: &'a str,
    format: Format,
    schema: Schema<'a>,
    http: HttpConfig,
    worker_binding: WorkerBindingConfig,
}

#[derive(Serialize)]
struct Format {
    #[serde(rename = "type")]
    format_type: &'static str,
}

#[derive(Serialize)]
struct Schema<'a> {
    fields: &'a [SchemaField],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SchemaField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Serialize)]
struct HttpConfig {
    enabled: bool,
    authentication: bool,
}

#[derive(Serialize)]
struct WorkerBindingConfig {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct Stream {
    pub id: String,
    pub name: String,
    pub endpoint: Option<String>,
}

// ============ Sinks ============

#[derive(Serialize)]
struct CreateSinkRequest<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    sink_type: &'static str,
    format: SinkFormat,
    config: SinkConfig<'a>,
}

#[derive(Serialize)]
struct SinkFormat {
    #[serde(rename = "type")]
    format_type: &'static str,
    compression: &'static str,
}

#[derive(Serialize)]
struct SinkConfig<'a> {
    bucket: &'a str,
    namespace: &'static str,
    table_name: &'a str,
    token: &'a str,
    rolling_policy: RollingPolicy,
}

#[derive(Serialize)]
struct RollingPolicy {
    /// Maximum file size in bytes before rollover
    file_size_bytes: u64,
    /// File write frequency in seconds (default: 300)
    interval_seconds: u32,
}

#[derive(Deserialize)]
pub struct Sink {
    pub id: String,
    pub name: String,
}

// ============ Pipelines ============

#[derive(Serialize)]
struct CreatePipelineRequest<'a> {
    name: &'a str,
    sql: String,
}

#[derive(Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub status: Option<String>,
}

impl CloudflareClient {
    // ============ Stream Methods ============

    /// List all streams
    pub async fn list_streams(&self) -> Result<Vec<Stream>> {
        self.get("/pipelines/v1/streams").await
    }

    /// Create a stream with the given schema
    pub async fn create_stream(
        &self,
        name: &str,
        schema: &[SchemaField],
    ) -> Result<Option<Stream>> {
        self.post_idempotent(
            "/pipelines/v1/streams",
            &CreateStreamRequest {
                name,
                format: Format {
                    format_type: "json",
                },
                schema: Schema { fields: schema },
                http: HttpConfig {
                    enabled: true,
                    authentication: true,
                },
                worker_binding: WorkerBindingConfig { enabled: true },
            },
        )
        .await
    }

    /// Delete a stream by ID
    pub async fn delete_stream(&self, id: &str) -> Result<()> {
        self.delete(&format!("/pipelines/v1/streams/{}", id)).await
    }

    // ============ Sink Methods ============

    /// List all sinks
    pub async fn list_sinks(&self) -> Result<Vec<Sink>> {
        self.get("/pipelines/v1/sinks").await
    }

    /// Create an R2 Data Catalog sink
    pub async fn create_sink(
        &self,
        name: &str,
        bucket: &str,
        table_name: &str,
        token: &str,
    ) -> Result<Option<Sink>> {
        self.post_idempotent(
            "/pipelines/v1/sinks",
            &CreateSinkRequest {
                name,
                sink_type: "r2_data_catalog",
                format: SinkFormat {
                    format_type: "parquet",
                    compression: "zstd",
                },
                config: SinkConfig {
                    bucket,
                    namespace: "default",
                    table_name,
                    token,
                    rolling_policy: RollingPolicy {
                        file_size_bytes: 256 * 1024 * 1024, // 256 MB
                        interval_seconds: 300,
                    },
                },
            },
        )
        .await
    }

    /// Delete a sink by ID
    pub async fn delete_sink(&self, id: &str) -> Result<()> {
        self.delete(&format!("/pipelines/v1/sinks/{}", id)).await
    }

    // ============ Pipeline Methods ============

    /// List all pipelines
    pub async fn list_pipelines(&self) -> Result<Vec<Pipeline>> {
        self.get("/pipelines/v1/pipelines").await
    }

    /// Create a pipeline connecting stream to sink
    pub async fn create_pipeline(
        &self,
        name: &str,
        stream: &str,
        sink: &str,
    ) -> Result<Option<Pipeline>> {
        self.post_idempotent(
            "/pipelines/v1/pipelines",
            &CreatePipelineRequest {
                name,
                sql: format!("INSERT INTO {} SELECT * FROM {}", sink, stream),
            },
        )
        .await
    }

    /// Delete a pipeline by ID
    pub async fn delete_pipeline(&self, id: &str) -> Result<()> {
        self.delete(&format!("/pipelines/v1/pipelines/{}", id))
            .await
    }
}
