//! Azure Function App CLI operations.

use anyhow::{Context, Result};
use std::process::Command;

#[allow(dead_code)]
pub struct FunctionAppCli;

#[allow(dead_code)]
impl FunctionAppCli {
    pub fn new() -> Self {
        Self
    }

    /// Check if Function App exists
    pub fn exists(&self, name: &str, rg: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "functionapp",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
            ])
            .output()
            .context("Failed to check Function App")?;

        Ok(result.status.success())
    }

    /// Get Function App URL
    pub fn get_url(&self, name: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "functionapp",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
                "--query",
                "defaultHostName",
                "-o",
                "tsv",
            ])
            .output()
            .context("Failed to get Function App URL")?;

        if !output.status.success() {
            return Ok("unknown".to_string());
        }

        let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(format!("https://{}", hostname))
    }

    /// Get Function App state
    pub fn get_state(&self, name: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "functionapp",
                "show",
                "--name",
                name,
                "--resource-group",
                rg,
                "--query",
                "state",
                "-o",
                "tsv",
            ])
            .output()
            .context("Failed to get Function App state")?;

        if !output.status.success() {
            return Ok("Unknown".to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Update Function App container image (for ghcr.io public images)
    pub fn set_container_image(&self, name: &str, rg: &str, image: &str) -> Result<()> {
        eprintln!("    Updating Function App to use image: {}", image);

        let output = Command::new("az")
            .args([
                "functionapp",
                "config",
                "container",
                "set",
                "--name",
                name,
                "--resource-group",
                rg,
                "--image",
                image,
                "--registry-server",
                "ghcr.io",
            ])
            .output()
            .context("Failed to update Function App container")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to set container image: {}", stderr);
        }

        Ok(())
    }

    /// Update Function App container image with ghcr.io credentials (for private images)
    pub fn set_container_image_with_auth(
        &self,
        name: &str,
        rg: &str,
        image: &str,
        username: &str,
        token: &str,
    ) -> Result<()> {
        eprintln!("    Updating Function App to use image: {}", image);

        let output = Command::new("az")
            .args([
                "functionapp",
                "config",
                "container",
                "set",
                "--name",
                name,
                "--resource-group",
                rg,
                "--image",
                image,
                "--registry-server",
                "ghcr.io",
                "--registry-username",
                username,
                "--registry-password",
                token,
            ])
            .output()
            .context("Failed to update Function App container")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to set container image: {}", stderr);
        }

        Ok(())
    }

    /// Set Function App configuration (environment variables)
    pub fn set_config(&self, name: &str, rg: &str, settings: &[(&str, &str)]) -> Result<()> {
        let settings_str: Vec<String> = settings
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let mut args = vec![
            "functionapp",
            "config",
            "appsettings",
            "set",
            "--name",
            name,
            "--resource-group",
            rg,
            "--settings",
        ];
        for s in &settings_str {
            args.push(s);
        }

        let output = Command::new("az")
            .args(&args)
            .output()
            .context("Failed to set Function App config")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to set config: {}", stderr);
        }

        Ok(())
    }

    /// Restart Function App
    pub fn restart(&self, name: &str, rg: &str) -> Result<()> {
        let output = Command::new("az")
            .args([
                "functionapp",
                "restart",
                "--name",
                name,
                "--resource-group",
                rg,
            ])
            .output()
            .context("Failed to restart Function App")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to restart: {}", stderr);
        }

        Ok(())
    }
}
