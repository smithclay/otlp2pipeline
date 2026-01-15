// src/cli/commands/azure/cli/storage.rs
use anyhow::{Context, Result};
use std::process::Command;

pub struct StorageCli;

impl StorageCli {
    pub fn new() -> Self {
        Self
    }

    /// Check if storage account exists
    pub fn account_exists(&self, name: &str, rg: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "storage",
                "account",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute Azure CLI to check if storage account '{}' exists in resource group '{}'",
                    name, rg
                )
            })?;

        if result.status.success() {
            return Ok(true);
        }

        // Parse stderr to distinguish "not found" from other errors
        let stderr = String::from_utf8(result.stderr)
            .context("Azure CLI returned invalid UTF-8 in error output")?;

        // Storage account not found is the expected "doesn't exist" case
        if stderr.contains("StorageAccountNotFound")
            || stderr.contains("ResourceNotFound")
            || stderr.contains("could not be found")
            || stderr.to_lowercase().contains("not found")
        {
            return Ok(false);
        }

        // Any other error should propagate with context
        anyhow::bail!(
            "Failed to check if storage account '{}' exists in resource group '{}': {}",
            name,
            rg,
            stderr.trim()
        );
    }

    /// Check if container exists
    pub fn container_exists(&self, container: &str, account: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "storage",
                "container",
                "show",
                "--name",
                container,
                "--account-name",
                account,
                "--auth-mode",
                "login",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute Azure CLI to check if container '{}' exists in storage account '{}'",
                    container, account
                )
            })?;

        if result.status.success() {
            return Ok(true);
        }

        // Parse stderr to distinguish "not found" from other errors
        let stderr = String::from_utf8(result.stderr)
            .context("Azure CLI returned invalid UTF-8 in error output")?;

        // Container not found is the expected "doesn't exist" case
        if stderr.contains("ContainerNotFound")
            || stderr.contains("ResourceNotFound")
            || stderr.contains("could not be found")
            || stderr.contains("can not be found")
            || stderr.to_lowercase().contains("not found")
        {
            return Ok(false);
        }

        // Any other error should propagate with context
        anyhow::bail!(
            "Failed to check if container '{}' exists in storage account '{}': {}",
            container,
            account,
            stderr.trim()
        );
    }

    /// Get storage account connection string
    pub fn get_connection_string(&self, account: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "storage",
                "account",
                "show-connection-string",
                "--name",
                account,
                "--resource-group",
                rg,
                "--query",
                "connectionString",
                "-o",
                "tsv",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to get connection string for storage account '{}' in resource group '{}'",
                    account, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!(
                "Failed to get connection string for storage account '{}': {}",
                account,
                stderr.trim()
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Azure CLI returned invalid UTF-8 in output")?;
        Ok(stdout.trim().to_string())
    }
}
