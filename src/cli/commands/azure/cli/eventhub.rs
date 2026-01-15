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
                    "Failed to execute Azure CLI to check if Event Hub namespace '{}' exists in resource group '{}'",
                    namespace, rg
                )
            })?;

        if result.status.success() {
            return Ok(true);
        }

        // Parse stderr to distinguish "not found" from other errors
        let stderr = String::from_utf8(result.stderr)
            .context("Azure CLI returned invalid UTF-8 in error output")?;

        // Namespace not found is the expected "doesn't exist" case
        if stderr.contains("NamespaceNotFound")
            || stderr.contains("ResourceNotFound")
            || stderr.contains("could not be found")
            || stderr.to_lowercase().contains("not found")
        {
            return Ok(false);
        }

        // Any other error should propagate with context
        anyhow::bail!(
            "Failed to check if Event Hub namespace '{}' exists in resource group '{}': {}",
            namespace,
            rg,
            stderr.trim()
        );
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
                    "Failed to execute Azure CLI to check if Event Hub '{}' exists in namespace '{}' (resource group '{}')",
                    hub, namespace, rg
                )
            })?;

        if result.status.success() {
            return Ok(true);
        }

        // Parse stderr to distinguish "not found" from other errors
        let stderr = String::from_utf8(result.stderr)
            .context("Azure CLI returned invalid UTF-8 in error output")?;

        // Event Hub not found is the expected "doesn't exist" case
        if stderr.contains("EventHubNotFound")
            || stderr.contains("ResourceNotFound")
            || stderr.contains("could not be found")
            || stderr.to_lowercase().contains("not found")
        {
            return Ok(false);
        }

        // Any other error should propagate with context
        anyhow::bail!(
            "Failed to check if Event Hub '{}' exists in namespace '{}' (resource group '{}'): {}",
            hub,
            namespace,
            rg,
            stderr.trim()
        );
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!(
                "Failed to get connection string for Event Hub namespace '{}': {}",
                namespace,
                stderr.trim()
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Azure CLI returned invalid UTF-8 in output")?;
        Ok(stdout.trim().to_string())
    }
}
