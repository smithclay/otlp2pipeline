use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

use crate::cli::config::{normalize_provider, Config, CONFIG_FILENAME};

pub struct InitArgs {
    pub provider: String,
    pub env: String,
    pub worker_url: Option<String>,
    pub region: Option<String>,
    pub force: bool,
}

/// Auto-detect AWS account ID using STS
fn detect_aws_account_id() -> Option<String> {
    let output = Command::new("aws")
        .args([
            "sts",
            "get-caller-identity",
            "--query",
            "Account",
            "--output",
            "text",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let account_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !account_id.is_empty() {
            return Some(account_id);
        }
    }
    None
}

pub fn execute_init(args: InitArgs) -> Result<()> {
    // Check if config already exists
    if Path::new(CONFIG_FILENAME).exists() && !args.force {
        bail!(
            "{} already exists. Use --force to overwrite.",
            CONFIG_FILENAME
        );
    }

    // Validate and normalize provider
    let provider = normalize_provider(&args.provider)?;

    // Provider-specific validation
    let (region, account_id) = if provider == "aws" {
        let region = args.region.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "AWS provider requires --region flag.\n\
                Example: otlp2pipeline init --provider aws --env prod --region us-east-1"
            )
        })?;

        // Auto-detect account ID
        eprintln!("Detecting AWS account ID...");
        let account_id = detect_aws_account_id();
        if let Some(ref id) = account_id {
            eprintln!("  Found: {}", id);
        } else {
            eprintln!("  Could not auto-detect (AWS CLI not configured or not installed)");
        }

        (Some(region), account_id)
    } else {
        (None, None)
    };

    let config = Config {
        provider,
        environment: args.env.clone(),
        worker_url: args.worker_url,
        account_id,
        region,
        stack_name: None,
    };

    config.save()?;

    eprintln!();
    eprintln!("Created {}", CONFIG_FILENAME);
    eprintln!("  provider: {}", config.provider);
    eprintln!("  environment: {}", config.environment);
    if let Some(ref url) = config.worker_url {
        eprintln!("  worker_url: {}", url);
    }
    if let Some(ref region) = config.region {
        eprintln!("  region: {}", region);
    }
    if let Some(ref account_id) = config.account_id {
        eprintln!("  account_id: {}", account_id);
    }
    eprintln!();

    match config.provider.as_str() {
        "cloudflare" => {
            eprintln!("Next: otlp2pipeline create --r2-token $R2_API_TOKEN --output wrangler.toml");
        }
        "aws" => {
            eprintln!("Next: otlp2pipeline create --output template.yaml");
        }
        _ => {
            eprintln!("Next: otlp2pipeline create");
        }
    }

    Ok(())
}
