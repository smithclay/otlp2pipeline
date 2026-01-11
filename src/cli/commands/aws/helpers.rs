use anyhow::{bail, Result};
use std::path::Path;

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

const ENV_REQUIRED_ERROR: &str = "No environment specified. Either:\n  \
    1. Run `otlp2pipeline init --provider aws --env <name>` first\n  \
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

/// Resolve environment name from args with an already-loaded config
pub fn resolve_env_with_config(env_arg: Option<String>, config: &Option<Config>) -> Result<String> {
    env_arg
        .or_else(|| config.as_ref().map(|c| c.environment.clone()))
        .ok_or_else(|| anyhow::anyhow!(ENV_REQUIRED_ERROR))
}

const DEFAULT_REGION: &str = "us-east-1";

/// Resolve region from args or config, warning if falling back to default
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

/// Generate stack name from environment, using naming normalization
pub fn stack_name(env: &str) -> String {
    format!("otlp2pipeline-{}", naming::normalize(env))
}

/// Validate that stack name won't exceed S3 bucket name limits
/// Error bucket format: ${STACK}-errors-${ACCOUNT_ID}-${REGION}
/// With 12-char account ID, stack must allow for 35 additional chars
pub fn validate_name_lengths(stack: &str, region: &str) -> Result<()> {
    let stack_len = stack.len();
    // Error bucket: stack + "-errors-" (8) + account_id (12) + "-" (1) + region
    let max_stack_len = 63 - 8 - 12 - 1 - region.len();

    if stack_len > max_stack_len {
        let error_bucket_len = stack_len + 8 + 12 + 1 + region.len();
        bail!(
            "Stack name '{}' is too long ({} chars)\n\
            Error bucket would be {} chars (max 63)\n\
            Max stack name length for region {}: {} chars\n\n\
            Use a shorter --env name",
            stack,
            stack_len,
            error_bucket_len,
            region,
            max_stack_len
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
            namespace: None,
            auth_token: None,
        });
        let region = resolve_region(None, &config);
        assert_eq!(region, "ap-southeast-1");
    }
}
