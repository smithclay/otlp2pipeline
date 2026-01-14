// src/cli/commands/azure/cli/az.rs
use anyhow::{Context, Result};
use std::process::Command;

/// Execute az CLI command and return stdout
fn run_az(args: &[&str]) -> Result<String> {
    let output = Command::new("az")
        .args(args)
        .output()
        .context("Failed to execute az command. Is Azure CLI installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("az command failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Azure CLI wrapper struct
pub struct AzureCli {
    region: String,
}

impl AzureCli {
    pub fn new(region: &str) -> Self {
        Self {
            region: region.to_string(),
        }
    }

    pub fn account(&self) -> AccountCli {
        AccountCli::new()
    }

    pub fn resource(&self) -> ResourceCli {
        ResourceCli::new(&self.region)
    }

    // TODO: uncomment in Task 3
    // pub fn storage(&self) -> StorageCli {
    //     StorageCli::new()
    // }

    // pub fn eventhub(&self) -> EventHubCli {
    //     EventHubCli::new()
    // }

    // pub fn stream_analytics(&self) -> StreamAnalyticsCli {
    //     StreamAnalyticsCli::new(&self.region)
    // }
}

/// Account operations (subscription info)
pub struct AccountCli;

impl AccountCli {
    pub fn new() -> Self {
        Self
    }

    /// Get current subscription ID
    pub fn get_subscription_id(&self) -> Result<String> {
        run_az(&["account", "show", "--query", "id", "-o", "tsv"])
            .context("Failed to get subscription ID. Run 'az login' first.")
    }
}

use super::ResourceCli;
// TODO: uncomment in Task 3
// use super::{EventHubCli, StorageCli, StreamAnalyticsCli};
