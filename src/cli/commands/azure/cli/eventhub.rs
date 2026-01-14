// src/cli/commands/azure/cli/eventhub.rs
use anyhow::{Context, Result};
use std::process::Command;

pub struct EventHubCli;

impl EventHubCli {
    pub fn new() -> Self {
        Self
    }

    /// Check if Event Hub namespace exists
    pub fn namespace_exists(&self, namespace: &str, rg: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "eventhubs",
                "namespace",
                "show",
                "--name",
                namespace,
                "--resource-group",
                rg,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to check if Event Hub namespace '{}' exists in resource group '{}'",
                    namespace, rg
                )
            })?;

        Ok(result.status.success())
    }

    /// Check if Event Hub exists
    pub fn hub_exists(&self, namespace: &str, hub: &str, rg: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "eventhubs",
                "eventhub",
                "show",
                "--name",
                hub,
                "--namespace-name",
                namespace,
                "--resource-group",
                rg,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to check if Event Hub '{}' exists in namespace '{}' (resource group '{}')",
                    hub, namespace, rg
                )
            })?;

        Ok(result.status.success())
    }

    /// Get Event Hub connection string
    pub fn get_connection_string(&self, namespace: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "eventhubs",
                "namespace",
                "authorization-rule",
                "keys",
                "list",
                "--resource-group",
                rg,
                "--namespace-name",
                namespace,
                "--name",
                "RootManageSharedAccessKey",
                "--query",
                "primaryConnectionString",
                "-o",
                "tsv",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to get connection string for Event Hub namespace '{}' in resource group '{}'",
                    namespace, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to get connection string for Event Hub namespace '{}': {}",
                namespace,
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
