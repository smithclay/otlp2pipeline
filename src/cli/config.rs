use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILENAME: &str = ".otlp2pipeline.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: String,
    pub environment: String,
    #[serde(default)]
    pub worker_url: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
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
        other => anyhow::bail!(
            "Provider '{}' not yet supported. Available: cloudflare",
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
    fn test_validate_provider_unknown() {
        let result = validate_provider("aws");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not yet supported"));
    }
}
