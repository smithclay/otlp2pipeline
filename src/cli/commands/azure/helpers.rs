// src/cli/commands/azure/helpers.rs
use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::cli::commands::naming;
use crate::cli::config::{Config, CONFIG_FILENAME};

/// Load config, distinguishing between "file not found" and "file invalid"
pub fn load_config() -> Result<Option<Config>> {
    if !Path::new(CONFIG_FILENAME).exists() {
        return Ok(None);
    }
    Config::load().map(Some)
}

const ENV_REQUIRED_ERROR: &str = "No environment specified. Either:\n  \
    1. Run `otlp2pipeline init --provider azure --env <name>` first\n  \
    2. Pass --env <name> explicitly";

/// Resolve environment name from args or config
pub fn resolve_env_name(env_arg: Option<String>) -> Result<String> {
    if let Some(env) = env_arg {
        return Ok(env);
    }

    match load_config()? {
        Some(config) => Ok(config.environment),
        None => bail!(ENV_REQUIRED_ERROR),
    }
}

/// Resolve environment name with already-loaded config
pub fn resolve_env_with_config(env_arg: Option<String>, config: &Option<Config>) -> Result<String> {
    env_arg
        .or_else(|| config.as_ref().map(|c| c.environment.clone()))
        .ok_or_else(|| anyhow::anyhow!(ENV_REQUIRED_ERROR))
}

const DEFAULT_REGION: &str = "westus";

/// Resolve region from args or config
pub fn resolve_region(region_arg: Option<String>, config: &Option<Config>) -> String {
    region_arg
        .or_else(|| config.as_ref().and_then(|c| c.region.clone()))
        .unwrap_or_else(|| {
            eprintln!(
                "    Note: No region specified, using default: {}",
                DEFAULT_REGION
            );
            DEFAULT_REGION.to_string()
        })
}

/// Resolve resource group name from args or generate from env
pub fn resolve_resource_group(rg_arg: &Option<String>, env_name: &str) -> String {
    rg_arg
        .clone()
        .unwrap_or_else(|| resource_group_name(env_name))
}

/// Generate resource group name
pub fn resource_group_name(env: &str) -> String {
    format!("otlp2pipeline-{}", naming::normalize(env))
}

/// Generate storage account name (Azure constraint: 3-24 chars, lowercase, no hyphens)
pub fn storage_account_name(env: &str) -> Result<String> {
    let normalized = naming::normalize(env).replace('-', "").to_lowercase();
    let name = format!("otlp{}adls", normalized);

    if name.len() > 24 {
        bail!(
            "Storage account name '{}' is too long ({} chars, max 24)\n\
            Environment name '{}' is too long. Use a shorter name.",
            name,
            name.len(),
            env
        );
    }

    if name.len() < 3 {
        bail!(
            "Storage account name '{}' is too short ({} chars, min 3)",
            name,
            name.len()
        );
    }

    Ok(name)
}

/// Generate Event Hub namespace name
pub fn eventhub_namespace(env: &str) -> String {
    format!("otlp-{}-hub", naming::normalize(env))
}

/// Event Hub name (constant)
pub const EVENTHUB_NAME: &str = "otlp-ingestion";

/// Generate Stream Analytics job name
pub fn stream_analytics_job_name(env: &str) -> String {
    format!("otlp-{}-stream-processor", naming::normalize(env))
}

/// Container names for ADLS Gen2
pub const CONTAINERS: &[&str] = &["logs", "traces", "metrics-gauge", "metrics-sum"];

/// Validate name lengths before deployment
pub fn validate_name_lengths(env: &str, _region: &str) -> Result<()> {
    let storage = storage_account_name(env)
        .context("Storage account name validation failed")?;

    let resource_group = resource_group_name(env);
    if resource_group.len() > 90 {
        bail!(
            "Resource group name '{}' is too long ({} chars, max 90)\n\
            Use a shorter environment name.",
            resource_group,
            resource_group.len()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_account_name() {
        assert_eq!(storage_account_name("prod").unwrap(), "otlpprodadls");
        assert_eq!(storage_account_name("test-01").unwrap(), "otlptest01adls");
        assert_eq!(
            storage_account_name("otlp2pipeline-staging").unwrap(),
            "otlpstagingadls"
        );
    }

    #[test]
    fn test_storage_account_name_too_long() {
        let result = storage_account_name("very-long-environment-name-that-exceeds-limits");
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_group_name() {
        assert_eq!(resource_group_name("prod"), "otlp2pipeline-prod");
        assert_eq!(
            resource_group_name("otlp2pipeline-test"),
            "otlp2pipeline-test"
        );
    }

    #[test]
    fn test_eventhub_namespace() {
        assert_eq!(eventhub_namespace("prod"), "otlp-prod-hub");
        assert_eq!(eventhub_namespace("test-01"), "otlp-test-01-hub");
    }

    #[test]
    fn test_stream_analytics_job_name() {
        assert_eq!(
            stream_analytics_job_name("prod"),
            "otlp-prod-stream-processor"
        );
    }
}
