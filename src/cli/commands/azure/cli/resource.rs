// src/cli/commands/azure/cli/resource.rs
use anyhow::{Context, Result};
use std::process::Command;

pub struct ResourceCli {
    region: String,
}

impl ResourceCli {
    pub fn new(region: &str) -> Self {
        Self {
            region: region.to_string(),
        }
    }

    /// Check if resource group exists
    pub fn group_exists(&self, name: &str) -> Result<bool> {
        let result = Command::new("az")
            .args(["group", "show", "--name", name])
            .output()
            .with_context(|| format!("Failed to check if resource group '{}' exists", name))?;

        Ok(result.status.success())
    }

    /// Create resource group
    pub fn create_group(&self, name: &str) -> Result<()> {
        let output = Command::new("az")
            .args([
                "group",
                "create",
                "--name",
                name,
                "--location",
                &self.region,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to create resource group '{}' in region '{}'",
                    name, self.region
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!(
                "Failed to create resource group '{}': {}",
                name,
                stderr.trim()
            );
        }

        Ok(())
    }

    /// Delete resource group
    pub fn delete_group(&self, name: &str) -> Result<()> {
        eprintln!(
            "    Deleting resource group: {} (this may take several minutes)",
            name
        );

        let output = Command::new("az")
            .args(["group", "delete", "--name", name, "--yes", "--no-wait"])
            .output()
            .with_context(|| format!("Failed to delete resource group '{}'", name))?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!(
                "Failed to delete resource group '{}': {}",
                name,
                stderr.trim()
            );
        }

        Ok(())
    }

    /// Deploy Bicep template
    pub fn deploy_bicep(
        &self,
        rg: &str,
        template_path: &str,
        params: &[(&str, &str)],
    ) -> Result<()> {
        let mut args = vec![
            "deployment",
            "group",
            "create",
            "--resource-group",
            rg,
            "--template-file",
            template_path,
        ];

        // Add parameters
        let param_strings: Vec<String> =
            params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();

        if !param_strings.is_empty() {
            args.push("--parameters");
            for param in &param_strings {
                args.push(param);
            }
        }

        let output = Command::new("az").args(&args).output().with_context(|| {
            format!("Failed to deploy Bicep template to resource group '{}'", rg)
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!(
                "Bicep deployment failed for resource group '{}': {}",
                rg,
                stderr.trim()
            );
        }

        Ok(())
    }
}
