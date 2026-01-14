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
            .context("Failed to check Event Hub namespace")?;

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
            .context("Failed to check Event Hub")?;

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
            .context("Failed to get Event Hub connection string")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to get connection string: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
