use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const CATALOG_BASE: &str = "https://catalog.cloudflarestorage.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Iceberg REST Catalog client for Cloudflare R2
pub struct IcebergClient {
    client: Client,
    token: String,
    account_id: String,
    bucket: String,
    /// Warehouse prefix (UUID) obtained from config endpoint
    prefix: Option<String>,
}

/// Response from the Iceberg catalog config endpoint
#[derive(Debug, Deserialize)]
struct CatalogConfig {
    overrides: Option<CatalogOverrides>,
}

#[derive(Debug, Deserialize)]
struct CatalogOverrides {
    prefix: Option<String>,
}

/// Table metadata from Iceberg REST API
#[derive(Debug, Deserialize)]
pub struct TableMetadata {
    #[serde(rename = "metadata-location")]
    pub metadata_location: Option<String>,
    pub metadata: TableMetadataInner,
}

#[derive(Debug, Deserialize)]
pub struct TableMetadataInner {
    #[serde(rename = "table-uuid")]
    pub table_uuid: Option<String>,
    pub location: Option<String>,
    #[serde(rename = "current-schema-id")]
    pub current_schema_id: Option<i32>,
    #[serde(rename = "last-updated-ms")]
    pub last_updated_ms: Option<i64>,
    #[serde(default)]
    pub schemas: Vec<Schema>,
    #[serde(rename = "partition-specs", default)]
    pub partition_specs: Vec<PartitionSpec>,
    #[serde(rename = "default-spec-id")]
    pub default_spec_id: Option<i32>,
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
}

#[derive(Debug, Deserialize)]
pub struct Schema {
    #[serde(rename = "schema-id")]
    pub schema_id: i32,
    #[serde(default)]
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Deserialize)]
pub struct SchemaField {
    pub id: i32,
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: serde_json::Value,
    pub required: bool,
}

#[derive(Debug, Deserialize)]
pub struct PartitionSpec {
    #[serde(rename = "spec-id")]
    pub spec_id: i32,
    #[serde(default)]
    pub fields: Vec<PartitionField>,
}

#[derive(Debug, Deserialize)]
pub struct PartitionField {
    #[serde(rename = "source-id")]
    pub source_id: i32,
    #[serde(rename = "field-id")]
    pub field_id: i32,
    pub name: String,
    pub transform: String,
}

#[derive(Debug, Deserialize)]
pub struct Snapshot {
    #[serde(rename = "snapshot-id")]
    pub snapshot_id: i64,
}

impl IcebergClient {
    /// Create a new Iceberg REST catalog client
    pub fn new(token: String, account_id: String, bucket: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("frostbit-cli")
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            token,
            account_id,
            bucket,
            prefix: None,
        })
    }

    /// Build the catalog base URL (account_id/bucket format)
    fn catalog_base_url(&self) -> String {
        format!("{}/{}/{}", CATALOG_BASE, self.account_id, self.bucket)
    }

    /// Fetch the catalog config to get the warehouse prefix (UUID).
    /// This must be called before get_table_metadata.
    pub async fn fetch_config(&mut self) -> Result<()> {
        let warehouse = format!("{}_{}", self.account_id, self.bucket);
        let url = format!(
            "{}/v1/config?warehouse={}",
            self.catalog_base_url(),
            warehouse
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .context("Failed to fetch catalog config")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to fetch catalog config: HTTP {} - {}", status, body);
        }

        let config: CatalogConfig = response
            .json()
            .await
            .context("Failed to parse catalog config")?;

        self.prefix = config.overrides.and_then(|o| o.prefix).or({
            // Some catalogs may not have overrides.prefix, use warehouse as fallback
            None
        });

        if self.prefix.is_none() {
            bail!("Catalog config does not contain a warehouse prefix");
        }

        Ok(())
    }

    /// Build the table URL using the prefix from config
    fn table_url(&self, table: &str) -> Result<String> {
        let prefix = self
            .prefix
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Catalog prefix not set. Call fetch_config() first."))?;

        Ok(format!(
            "{}/v1/{}/namespaces/default/tables/{}",
            self.catalog_base_url(),
            prefix,
            table
        ))
    }

    /// Get table metadata, returns None if table doesn't exist.
    /// Requires fetch_config() to be called first.
    pub async fn get_table_metadata(&self, table: &str) -> Result<Option<TableMetadata>> {
        let url = self.table_url(table)?;

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .with_context(|| format!("Failed to fetch metadata for table '{}'", table))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!(
                "Failed to fetch table '{}': HTTP {} - {}",
                table,
                status,
                body
            );
        }

        let metadata = response
            .json::<TableMetadata>()
            .await
            .with_context(|| format!("Failed to parse metadata for table '{}'", table))?;

        Ok(Some(metadata))
    }
}

impl TableMetadataInner {
    /// Get the current schema
    fn current_schema(&self) -> Option<&Schema> {
        let current_id = self.current_schema_id.unwrap_or(0);
        self.schemas.iter().find(|s| s.schema_id == current_id)
    }

    /// Get field names as comma-separated preview
    pub fn field_names_preview(&self, max_shown: usize) -> String {
        let Some(schema) = self.current_schema() else {
            return String::new();
        };

        let total = schema.fields.len();
        let names: Vec<&str> = schema
            .fields
            .iter()
            .take(max_shown)
            .map(|f| f.name.as_str())
            .collect();

        if total > max_shown {
            format!("{}, ... ({} total)", names.join(", "), total)
        } else {
            names.join(", ")
        }
    }

    /// Format partition specs for display
    pub fn format_partition_specs(&self) -> Vec<String> {
        let default_id = self.default_spec_id.unwrap_or(0);

        self.partition_specs
            .iter()
            .map(|spec| {
                let is_default = spec.spec_id == default_id;
                let default_marker = if is_default { " (default)" } else { "" };

                if spec.fields.is_empty() {
                    format!(
                        "spec-id: {}{} - unpartitioned",
                        spec.spec_id, default_marker
                    )
                } else {
                    let transforms: Vec<String> = spec
                        .fields
                        .iter()
                        .map(|f| format!("{}({})", f.transform, f.name))
                        .collect();
                    format!(
                        "spec-id: {}{} - {}",
                        spec.spec_id,
                        default_marker,
                        transforms.join(", ")
                    )
                }
            })
            .collect()
    }

    /// Format last updated time
    pub fn format_last_updated(&self) -> String {
        match self.last_updated_ms {
            Some(ms) => {
                let secs = ms / 1000;
                let nanos = ((ms % 1000) * 1_000_000) as u32;
                chrono::DateTime::from_timestamp(secs, nanos)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }
            None => "unknown".to_string(),
        }
    }
}
