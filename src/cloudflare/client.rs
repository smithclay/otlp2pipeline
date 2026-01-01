use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare API client
pub struct CloudflareClient {
    client: Client,
    token: String,
    account_id: String,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    success: bool,
    result: Option<T>,
    errors: Vec<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Deserialize)]
struct Account {
    id: String,
}

impl CloudflareClient {
    /// Create a new client, auto-detecting account ID if not provided
    pub async fn new(token: String, account_id: Option<String>) -> Result<Self> {
        let client = Client::builder().user_agent("otlpflare-cli").build()?;

        let account_id = match account_id {
            Some(id) => id,
            None => Self::detect_account_id(&client, &token).await?,
        };

        Ok(Self {
            client,
            token,
            account_id,
        })
    }

    async fn detect_account_id(client: &Client, token: &str) -> Result<String> {
        let response: ApiResponse<Vec<Account>> = client
            .get(format!("{}/accounts", API_BASE))
            .bearer_auth(token)
            .send()
            .await?
            .json()
            .await?;

        if !response.success {
            let msg = response
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("Unknown error");
            bail!("Failed to detect account: {}", msg);
        }

        response
            .result
            .and_then(|accounts| accounts.into_iter().next())
            .map(|a| a.id)
            .ok_or_else(|| anyhow::anyhow!("No accounts found"))
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    /// GET request to Cloudflare API
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}/accounts/{}{}", API_BASE, self.account_id, path);
        let response: ApiResponse<T> = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .with_context(|| format!("GET {}", path))?
            .json()
            .await?;

        if !response.success {
            let msg = response
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("Unknown error");
            bail!("API error: {}", msg);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("Empty result"))
    }

    /// POST request to Cloudflare API
    pub async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}/accounts/{}{}", API_BASE, self.account_id, path);
        let response: ApiResponse<T> = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {}", path))?
            .json()
            .await?;

        if !response.success {
            let msg = response
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("Unknown error");
            bail!("API error: {}", msg);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("Empty result"))
    }

    /// POST request that may return 409 (conflict) for idempotent creates
    pub async fn post_idempotent<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<Option<T>> {
        let url = format!("{}/accounts/{}{}", API_BASE, self.account_id, path);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {}", path))?;

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return Ok(None); // Resource already exists
        }

        let response: ApiResponse<T> = resp.json().await?;

        if !response.success {
            let msg = response
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("Unknown error");
            bail!("API error: {}", msg);
        }

        Ok(response.result)
    }

    /// DELETE request to Cloudflare API
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}/accounts/{}{}", API_BASE, self.account_id, path);
        let response: ApiResponse<serde_json::Value> = self
            .client
            .delete(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .with_context(|| format!("DELETE {}", path))?
            .json()
            .await?;

        if !response.success {
            let msg = response
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("Unknown error");
            bail!("API error: {}", msg);
        }

        Ok(())
    }
}
