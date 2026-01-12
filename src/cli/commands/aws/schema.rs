use anyhow::{bail, Result};

#[derive(Debug)]
pub struct Schema {
    pub fields: Vec<SchemaField>,
}

#[derive(Debug)]
pub struct SchemaField {
    pub name: String,
    pub field_type: String,
}

impl Schema {
    /// Load schema from otlp2records definitions
    pub fn load(table: &str) -> Result<Self> {
        let schema_name = match table {
            "traces" => "spans",
            other => other,
        };

        let schema_def = match otlp2records::schema_def(schema_name) {
            Some(schema) => schema,
            None => bail!("missing otlp2records schema: {}", schema_name),
        };

        let fields: Vec<SchemaField> = schema_def
            .fields
            .iter()
            .map(|field| SchemaField {
                name: field.name.to_string(),
                field_type: field.field_type.to_string(),
            })
            .collect();

        if fields.is_empty() {
            bail!("otlp2records schema {} has no fields", schema_name);
        }

        Ok(Self { fields })
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
