//! Iceberg REST Catalog client for Cloudflare R2.

use anyhow::{bail, Context, Result};
use reqwest::Client;
use std::time::Duration;

pub use super::iceberg_types::*;

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

impl IcebergClient {
    /// Create a new Iceberg REST catalog client
    pub fn new(token: String, account_id: String, bucket: String) -> Result<Self> {
        // Validate R2 token format
        if token.is_empty() {
            bail!("R2 API token is required. Create one at: dash.cloudflare.com > R2 > Manage R2 API Tokens");
        }
        if token.len() < 20 {
            bail!("R2 API token appears too short. Verify you copied the complete token.");
        }
        if token.len() > 200 {
            bail!("R2 API token appears too long. Verify you copied only the token value.");
        }

        let client = Client::builder()
            .user_agent("otlp2pipeline-cli")
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

    /// Add a service_name identity partition to a table.
    /// Returns the result indicating success, already partitioned, or table not found.
    /// Requires fetch_config() to be called first.
    pub async fn add_partition_spec(
        &self,
        table: &str,
        mut retries: u32,
    ) -> Result<AddPartitionResult> {
        loop {
            // Fetch current table metadata (re-fetch on each retry to get updated state)
            let Some(metadata) = self.get_table_metadata(table).await? else {
                return Ok(AddPartitionResult::TableNotFound);
            };

            let inner = &metadata.metadata;

            // Check if already partitioned by service_name
            if inner.is_partitioned_by_service_name() {
                return Ok(AddPartitionResult::AlreadyPartitioned);
            }

            // Get required fields
            let table_uuid = inner
                .table_uuid
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Table '{}' has no UUID", table))?;

            let default_spec_id = inner.default_spec_id.unwrap_or(0);
            // Iceberg convention: partition field IDs start at 1000
            // If last_partition_id is missing, use 999 so first new field gets ID 1000
            let last_partition_id = inner.last_partition_id.unwrap_or(999);

            let service_name_field_id = inner
                .get_service_name_field_id()
                .ok_or_else(|| anyhow::anyhow!("Table '{}' has no service_name field", table))?;

            // Find the spec that has the day partition (may be current default or an older spec)
            // We want to preserve the day partition when adding service_name
            let day_partition_fields: Vec<NewPartitionField> = inner
                .partition_specs
                .iter()
                .find(|spec| spec.fields.iter().any(|f| f.transform == "day"))
                .map(|spec| {
                    spec.fields
                        .iter()
                        .filter(|f| f.transform == "day") // Only copy day partitions
                        .map(|f| NewPartitionField {
                            name: f.name.clone(),
                            transform: f.transform.clone(),
                            source_id: f.source_id,
                            field_id: f.field_id,
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Build commit request
            let new_spec_id = default_spec_id + 1;
            let new_partition_field_id = last_partition_id + 1;

            // New spec includes day partition(s) plus service_name
            let mut new_fields = day_partition_fields;
            new_fields.push(NewPartitionField {
                name: "service_name".to_string(),
                transform: "identity".to_string(),
                source_id: service_name_field_id,
                field_id: new_partition_field_id,
            });

            let commit_request = CommitRequest {
                requirements: vec![
                    CommitRequirement::AssertTableUuid {
                        uuid: table_uuid.clone(),
                    },
                    CommitRequirement::AssertDefaultSpecId { default_spec_id },
                ],
                updates: vec![
                    CommitUpdate::AddSpec {
                        spec: NewPartitionSpec {
                            spec_id: new_spec_id,
                            fields: new_fields,
                        },
                    },
                    CommitUpdate::SetDefaultSpec {
                        spec_id: new_spec_id,
                    },
                ],
            };

            // POST commit
            match self.try_commit_table(table, &commit_request).await {
                Ok(()) => return Ok(AddPartitionResult::Added),
                Err(CommitError::Conflict) if retries > 0 => {
                    retries -= 1;
                    eprintln!(
                        "    Conflict detected for '{}', retrying ({} retries left)...",
                        table, retries
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(CommitError::Conflict) => {
                    bail!("Failed to commit partition spec to table '{}': concurrency conflict after retries", table);
                }
                Err(CommitError::Other(e)) => return Err(e),
            }
        }
    }

    /// Attempt to commit changes to a table. Returns Conflict on 409, Other for other errors.
    async fn try_commit_table(
        &self,
        table: &str,
        request: &CommitRequest,
    ) -> Result<(), CommitError> {
        let url = self.table_url(table).map_err(CommitError::Other)?;

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(request)
            .send()
            .await
            .with_context(|| format!("Failed to commit partition spec to table '{}'", table))
            .map_err(CommitError::Other)?;

        let status = response.status();

        if status.is_success() {
            return Ok(());
        }

        if status == reqwest::StatusCode::CONFLICT {
            return Err(CommitError::Conflict);
        }

        let body = response.text().await.unwrap_or_default();
        Err(CommitError::Other(anyhow::anyhow!(
            "Failed to commit partition spec to table '{}': HTTP {} - {}",
            table,
            status,
            body
        )))
    }
}

/// Internal error type for commit operations
enum CommitError {
    Conflict,
    Other(anyhow::Error),
}
