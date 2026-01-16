//! Azure Container Apps CLI operations.

use anyhow::{Context, Result};
use std::process::Command;

#[allow(dead_code)]
pub struct ContainerAppCli;

#[allow(dead_code)]
impl ContainerAppCli {
    pub fn new() -> Self {
        Self
    }

    /// Check if Container App exists
    pub fn exists(&self, name: &str, rg: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "containerapp",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
            ])
            .output()
            .context("Failed to check Container App")?;

        Ok(result.status.success())
    }

    /// Get Container App URL
    pub fn get_url(&self, name: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "containerapp",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
                "--query",
                "properties.configuration.ingress.fqdn",
                "-o",
                "tsv",
            ])
            .output()
            .context("Failed to get Container App URL")?;

        if !output.status.success() {
            return Ok("unknown".to_string());
        }

        let fqdn = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(format!("https://{}", fqdn))
    }

    /// Get Container App state
    pub fn get_state(&self, name: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "containerapp",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
                "--query",
                "properties.runningStatus",
                "-o",
                "tsv",
            ])
            .output()
            .context("Failed to get Container App state")?;

        if !output.status.success() {
            return Ok("Unknown".to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
