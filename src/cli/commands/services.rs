use anyhow::Result;

use crate::cli::url::resolve_worker_url;
use crate::cli::ServicesArgs;

pub async fn execute_services(args: ServicesArgs) -> Result<()> {
    let base_url = resolve_worker_url(args.url.as_deref())?;
    let url = format!("{}/v1/services", base_url);

    let client = reqwest::Client::new();
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
