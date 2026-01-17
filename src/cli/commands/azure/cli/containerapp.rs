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
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to get Container App URL for '{}' in '{}': {}",
                name,
                rg,
                stderr.trim()
            );
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to get Container App state for '{}' in '{}': {}",
                name,
                rg,
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Update Container App environment variables
    pub fn update_environment_variables(
        &self,
        name: &str,
        rg: &str,
        env_vars: &[(&str, &str)],
    ) -> Result<()> {
        let mut args = vec![
            "containerapp",
            "update",
            "--name",
            name,
            "--resource-group",
            rg,
            "--set-env-vars",
        ];

        // Azure CLI expects each KEY=VALUE pair as a separate argument
        let env_pairs: Vec<String> = env_vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        for pair in &env_pairs {
            args.push(pair);
        }

        let output = Command::new("az")
            .args(&args)
            .output()
            .context("Failed to update Container App environment variables")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to update environment variables: {}", stderr);
        }

        Ok(())
    }
}
