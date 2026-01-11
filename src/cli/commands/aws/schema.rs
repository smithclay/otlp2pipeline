use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Schema {
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Deserialize)]
pub struct SchemaField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

impl Schema {
    /// Load schema from embedded file
    pub fn load(table: &str) -> Result<Self> {
        let schema_file = match table {
            "traces" => "schemas/spans.schema.json",
            other => &format!("schemas/{}.schema.json", other),
        };

        let content = std::fs::read_to_string(schema_file)
            .with_context(|| format!("Failed to read schema file: {}", schema_file))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse schema file: {}", schema_file))
    }

    /// Generate Athena DDL column definitions
    pub fn to_athena_columns(&self) -> String {
        self.fields
            .iter()
            .map(|f| {
                let athena_type = match f.field_type.as_str() {
                    "int64" => "bigint",
                    "int32" => "int",
                    "float64" => "double",
                    "bool" => "boolean",
                    "json" => "string",
                    other => other,
                };
                format!("{} {}", f.name, athena_type)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Generate CREATE TABLE DDL for S3 Tables
    pub fn to_create_table_ddl(&self, namespace: &str, table: &str) -> String {
        format!(
            "CREATE TABLE `{}`.{} (\n  {}\n)\nPARTITIONED BY (day(timestamp))",
            namespace,
            table,
            self.to_athena_columns()
        )
    }
}

/// Tables to create
pub const TABLES: &[&str] = &["logs", "traces", "sum", "gauge"];
