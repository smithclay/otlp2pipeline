//! Iceberg REST Catalog API types.

use serde::{Deserialize, Serialize};

/// Response from the Iceberg catalog config endpoint
#[derive(Debug, Deserialize)]
pub struct CatalogConfig {
    pub overrides: Option<CatalogOverrides>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogOverrides {
    pub prefix: Option<String>,
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
    #[serde(rename = "last-partition-id")]
    pub last_partition_id: Option<i32>,
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

/// Result of adding a partition spec
#[derive(Debug)]
pub enum AddPartitionResult {
    /// Partition spec was successfully added
    Added,
    /// Table already partitioned by service_name
    AlreadyPartitioned,
    /// Table does not exist
    TableNotFound,
}

/// Commit request for Iceberg table updates
#[derive(Debug, Serialize)]
pub struct CommitRequest {
    pub requirements: Vec<CommitRequirement>,
    pub updates: Vec<CommitUpdate>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CommitRequirement {
    #[serde(rename = "assert-table-uuid")]
    AssertTableUuid { uuid: String },
    #[serde(rename = "assert-default-spec-id")]
    AssertDefaultSpecId {
        #[serde(rename = "default-spec-id")]
        default_spec_id: i32,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum CommitUpdate {
    #[serde(rename = "add-spec")]
    AddSpec { spec: NewPartitionSpec },
    #[serde(rename = "set-default-spec")]
    SetDefaultSpec {
        #[serde(rename = "spec-id")]
        spec_id: i32,
    },
}

#[derive(Debug, Serialize)]
pub struct NewPartitionSpec {
    #[serde(rename = "spec-id")]
    pub spec_id: i32,
    pub fields: Vec<NewPartitionField>,
}

#[derive(Debug, Serialize)]
pub struct NewPartitionField {
    pub name: String,
    pub transform: String,
    #[serde(rename = "source-id")]
    pub source_id: i32,
    #[serde(rename = "field-id")]
    pub field_id: i32,
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

    /// Check if the default partition spec has BOTH service_name identity AND a day partition.
    /// Returns true only if both are present (properly partitioned).
    pub fn is_partitioned_by_service_name(&self) -> bool {
        let default_id = self.default_spec_id.unwrap_or(0);

        self.partition_specs
            .iter()
            .find(|spec| spec.spec_id == default_id)
            .is_some_and(|spec| {
                let has_service_name = spec
                    .fields
                    .iter()
                    .any(|f| f.name == "service_name" && f.transform == "identity");
                let has_day_partition = spec.fields.iter().any(|f| f.transform == "day");
                has_service_name && has_day_partition
            })
    }

    /// Get the field ID for service_name from the current schema
    pub fn get_service_name_field_id(&self) -> Option<i32> {
        self.current_schema()
            .and_then(|schema| schema.fields.iter().find(|f| f.name == "service_name"))
            .map(|f| f.id)
    }
}
