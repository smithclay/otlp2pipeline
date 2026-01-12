use super::{run_idempotent, run_optional, AwsCli};
use anyhow::Result;
use serde::Deserialize;
use std::process::Command;

pub struct GlueCli<'a> {
    pub(super) aws: &'a AwsCli,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GetTableResponse {
    table: TableInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TableInfo {
    pub name: String,
    pub parameters: Option<std::collections::HashMap<String, String>>,
}

impl GlueCli<'_> {
    pub fn delete_catalog(&self, catalog_id: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "glue",
            "delete-catalog",
            "--catalog-id",
            catalog_id,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &["EntityNotFoundException"])?;
        Ok(())
    }

    pub fn create_catalog(&self, name: &str, resource_arn: &str) -> Result<bool> {
        let catalog_input = serde_json::json!({
            "FederatedCatalog": {"Identifier": resource_arn, "ConnectionName": "aws:s3tables"},
            "CreateDatabaseDefaultPermissions": [],
            "CreateTableDefaultPermissions": [],
            "CatalogProperties": {"CustomProperties": {"AllowFullTableExternalDataAccess": "true"}}
        });
        let mut cmd = Command::new("aws");
        cmd.args([
            "glue",
            "create-catalog",
            "--name",
            name,
            "--catalog-input",
            &catalog_input.to_string(),
            "--region",
            self.aws.region(),
        ]);
        let output = run_idempotent(&mut cmd, &["AlreadyExistsException"])?;
        Ok(!output.already_existed)
    }

    pub fn catalog_exists(&self, catalog_id: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "glue",
            "get-catalog",
            "--catalog-id",
            catalog_id,
            "--region",
            self.aws.region(),
        ]);
        let result = run_optional(&mut cmd, &["EntityNotFoundException"])?;
        Ok(result.is_some())
    }

    pub fn get_table(
        &self,
        catalog_id: &str,
        database: &str,
        table: &str,
    ) -> Result<Option<TableInfo>> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "glue",
            "get-table",
            "--catalog-id",
            catalog_id,
            "--database-name",
            database,
            "--name",
            table,
            "--region",
            self.aws.region(),
            "--output",
            "json",
        ]);
        let result = run_optional(&mut cmd, &["EntityNotFoundException"])?;
        match result {
            Some(json) => {
                let response: GetTableResponse = serde_json::from_str(&json)?;
                Ok(Some(response.table))
            }
            None => Ok(None),
        }
    }
}
