use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;

/// Resolved Cloudflare credentials
pub struct Credentials {
    pub token: String,
    pub account_id: Option<String>,
}

#[derive(Deserialize)]
struct WranglerConfig {
    oauth_token: Option<String>,
    expiration_time: Option<String>,
}

/// Resolve Cloudflare API token from environment or wrangler config
pub fn resolve_credentials() -> Result<Credentials> {
    // Try CF_API_TOKEN first
    if let Ok(token) = env::var("CF_API_TOKEN") {
        let account_id = env::var("CF_ACCOUNT_ID").ok();
        return Ok(Credentials { token, account_id });
    }

    // Fall back to wrangler OAuth token
    let config_path = wrangler_config_path()?;
    let config_content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read wrangler config at {:?}", config_path))?;

    // Strip ANSI codes (wrangler sometimes writes colored output)
    let clean_content: String = config_content
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();

    let config: WranglerConfig =
        toml::from_str(&clean_content).context("Failed to parse wrangler config")?;

    let token = config.oauth_token.ok_or_else(|| {
        anyhow::anyhow!("No OAuth token in wrangler config. Run 'npx wrangler login'")
    })?;

    // Check expiration
    if let Some(exp) = config.expiration_time {
        if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(&exp) {
            if exp_time < chrono::Utc::now() {
                eprintln!("Warning: Wrangler OAuth token may be expired ({})", exp);
                eprintln!("Run 'npx wrangler login' to refresh");
            }
        }
    }

    let account_id = env::var("CF_ACCOUNT_ID").ok();
    Ok(Credentials { token, account_id })
}

fn wrangler_config_path() -> Result<PathBuf> {
    // macOS: ~/Library/Preferences/.wrangler/config/default.toml
    // Linux: ~/.wrangler/config/default.toml

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let macos_path = home.join("Library/Preferences/.wrangler/config/default.toml");
            if macos_path.exists() {
                return Ok(macos_path);
            }
        }
    }

    // Fallback: ~/.wrangler/config/default.toml
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".wrangler/config/default.toml");
        if path.exists() {
            return Ok(path);
        }
    }

    bail!("Wrangler config not found. Run 'npx wrangler login' or set CF_API_TOKEN")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrangler_config_path_exists_or_errors() {
        // Just verify the function doesn't panic
        let _ = wrangler_config_path();
    }
}
