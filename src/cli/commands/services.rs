use std::time::Duration;

use anyhow::{bail, Result};

use crate::cli::config::try_load_config;
use crate::cli::url::resolve_worker_url;
use crate::cli::ServicesArgs;

/// Default timeout for HTTP requests (30 seconds)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn execute_services(args: ServicesArgs) -> Result<()> {
    // Check if provider is AWS - services command is Cloudflare-only
    if let Some(config) = try_load_config() {
        if config.provider == "aws" {
            bail!(
                "The `services` command is only available for Cloudflare.\n\n\
                AWS Lambda does not support the service registry feature.\n\
                Use `otlp2pipeline aws status` to check your AWS deployment."
            );
        }
    }

    let base_url = resolve_worker_url(args.url.as_deref()).await?;
    let url = format!("{}/v1/services", base_url);

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch services: {} - {}", status, body);
    }

    let body = response.text().await?;
    println!("{}", body);

    Ok(())
}
