use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Resolved Cloudflare credentials
pub struct Credentials {
    pub token: String,
    pub account_id: Option<String>,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    success: bool,
    result: Option<T>,
    errors: Option<Vec<ApiError>>,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Deserialize)]
struct SubdomainResult {
    subdomain: String,
}

#[derive(Deserialize)]
struct AccountResult {
    id: String,
}

#[derive(Deserialize)]
struct AccountsResponse {
    result: Vec<AccountResult>,
}

/// Fetch the workers.dev subdomain for an account
pub async fn fetch_workers_subdomain(creds: &Credentials) -> Result<String> {
    let account_id = match &creds.account_id {
        Some(id) => id.clone(),
        None => fetch_account_id(&creds.token).await?,
    };

    let client = reqwest::Client::new();
    let url = format!("{}/accounts/{}/workers/subdomain", CF_API_BASE, account_id);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", creds.token))
        .send()
        .await
        .context("Failed to fetch workers subdomain")?;

    let api_response: ApiResponse<SubdomainResult> = response
        .json()
        .await
        .context("Failed to parse subdomain response")?;

    if !api_response.success {
        let errors = api_response
            .errors
            .map(|e| {
                e.iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "Unknown error".to_string());
        bail!("Failed to fetch workers subdomain: {}", errors);
    }

    api_response
        .result
        .map(|r| r.subdomain)
        .ok_or_else(|| anyhow::anyhow!("No subdomain in response"))
}

/// Fetch account ID from the API (uses first account if multiple)
async fn fetch_account_id(token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/accounts", CF_API_BASE);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to fetch accounts")?;

    let accounts: AccountsResponse = response
        .json()
        .await
        .context("Failed to parse accounts response")?;

    accounts
        .result
        .first()
        .map(|a| a.id.clone())
        .ok_or_else(|| anyhow::anyhow!("No accounts found for this token"))
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
