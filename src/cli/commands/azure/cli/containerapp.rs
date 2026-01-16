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
        ];

        // Build --set-env-vars argument
        let env_pairs: Vec<String> = env_vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let env_arg = env_pairs.join(" ");

        args.push("--set-env-vars");
        args.push(&env_arg);

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
