use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILENAME: &str = ".otlp2pipeline.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: String,
    pub environment: String,
    // Cloudflare-specific
    #[serde(default)]
    pub worker_url: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    // AWS-specific
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub stack_name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    // Shared
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        load_config_from_path(CONFIG_FILENAME)
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(CONFIG_FILENAME, content)?;
        Ok(())
    }

    /// Update only the auth_token field and save
    pub fn set_auth_token(&mut self, token: String) -> Result<()> {
        self.auth_token = Some(token);
        self.save()
    }
}

/// Generate a secure random auth token (32 bytes, base64 URL-safe, ~43 chars)
pub fn generate_auth_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn load_config_from_path(path: impl AsRef<Path>) -> Result<Config> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("Failed to read {}", path.as_ref().display()))?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn try_load_config() -> Option<Config> {
    Config::load().ok()
}

pub fn validate_provider(provider: &str) -> Result<&'static str> {
    match provider.to_lowercase().as_str() {
        "cloudflare" | "cf" => Ok("cloudflare"),
        "aws" => Ok("aws"),
        other => anyhow::bail!(
            "Provider '{}' not supported. Available: cloudflare, aws",
            other
        ),
    }
}

pub fn normalize_provider(provider: &str) -> Result<String> {
    validate_provider(provider).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
provider = "cloudflare"
environment = "prod"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.provider, "cloudflare");
        assert_eq!(config.environment, "prod");
        assert_eq!(config.worker_url, None);
        assert_eq!(config.account_id, None);
    }

    #[test]
    fn test_load_config_not_found() {
        let result = load_config_from_path("/nonexistent/.otlp2pipeline.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_from_string() {
        let toml = r#"
provider = "cloudflare"
environment = "staging"
worker_url = "https://my-worker.workers.dev"
account_id = "abc123"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.worker_url,
            Some("https://my-worker.workers.dev".to_string())
        );
        assert_eq!(config.account_id, Some("abc123".to_string()));
    }

    #[test]
    fn test_validate_provider_cloudflare() {
        assert!(validate_provider("cloudflare").is_ok());
        assert!(validate_provider("cf").is_ok());
    }

    #[test]
    fn test_validate_provider_aws() {
        assert!(validate_provider("aws").is_ok());
    }

    #[test]
    fn test_validate_provider_unknown() {
        let result = validate_provider("gcp");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }
}
