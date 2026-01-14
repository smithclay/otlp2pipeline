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
                    "Failed to check if storage account '{}' exists in resource group '{}'",
                    name, rg
                )
            })?;

        Ok(result.status.success())
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
                    "Failed to check if container '{}' exists in storage account '{}'",
                    container, account
                )
            })?;

        Ok(result.status.success())
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to get connection string for storage account '{}': {}",
                account,
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
