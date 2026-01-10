use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

use crate::cli::commands::naming;
use crate::cli::config::{Config, CONFIG_FILENAME};

/// Load config, distinguishing between "file not found" and "file invalid"
pub fn load_config() -> Result<Option<Config>> {
    if !Path::new(CONFIG_FILENAME).exists() {
        return Ok(None);
    }
    // File exists, so errors are real problems (malformed TOML, permission denied, etc.)
    Config::load().map(Some)
}

/// Resolve environment name from args or config
pub fn resolve_env_name(env_arg: Option<String>) -> Result<String> {
    if let Some(env) = env_arg {
        return Ok(env);
    }

    match load_config()? {
        Some(config) => Ok(config.environment),
        None => bail!(
            "No environment specified. Either:\n  \
            1. Run `otlp2pipeline init --provider aws --env <name>` first\n  \
            2. Pass --env <name> explicitly"
        ),
    }
}

/// Resolve region from args or config, warning if falling back to default
pub fn resolve_region(region_arg: Option<String>, config: &Option<Config>) -> String {
    if let Some(region) = region_arg {
        return region;
    }

    if let Some(config) = config {
        if let Some(region) = &config.region {
            return region.clone();
        }
    }

    // Warn user about fallback
    eprintln!("    Note: No region specified, using default: us-east-1");
    "us-east-1".to_string()
}

/// Generate stack name from environment, using naming normalization
pub fn stack_name(env: &str) -> String {
    format!("otlp2pipeline-{}", naming::normalize(env))
}

/// Check if AWS CLI is available, with helpful error message
pub fn require_aws_cli(stack_name: &str, region: &str, fallback_command: &str) -> Result<()> {
    if Command::new("aws").arg("--version").output().is_err() {
        bail!(
            "AWS CLI not found. Install it from https://aws.amazon.com/cli/\n\n\
            Or run manually:\n  aws cloudformation {} --stack-name {} --region {}",
            fallback_command,
            stack_name,
            region
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_name_without_prefix() {
        assert_eq!(stack_name("prod"), "otlp2pipeline-prod");
    }

    #[test]
    fn test_stack_name_with_prefix() {
        // Should normalize to avoid double-prefix
        assert_eq!(stack_name("otlp2pipeline-prod"), "otlp2pipeline-prod");
    }

    #[test]
    fn test_stack_name_with_underscore_prefix() {
        assert_eq!(stack_name("otlp2pipeline_staging"), "otlp2pipeline-staging");
    }

    #[test]
    fn test_resolve_env_name_with_arg() {
        let result = resolve_env_name(Some("test-env".to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-env");
    }

    #[test]
    fn test_resolve_region_with_arg() {
        let region = resolve_region(Some("eu-west-1".to_string()), &None);
        assert_eq!(region, "eu-west-1");
    }

    #[test]
    fn test_resolve_region_from_config() {
        let config = Some(Config {
            provider: "aws".to_string(),
            environment: "prod".to_string(),
            worker_url: None,
            account_id: None,
            region: Some("ap-southeast-1".to_string()),
            stack_name: None,
        });
        let region = resolve_region(None, &config);
        assert_eq!(region, "ap-southeast-1");
    }
}
