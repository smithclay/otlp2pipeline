use anyhow::{bail, Result};
use std::path::Path;

use crate::cli::config::{normalize_provider, Config, CONFIG_FILENAME};

pub struct InitArgs {
    pub provider: String,
    pub env: String,
    pub worker_url: Option<String>,
    pub force: bool,
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

    let config = Config {
        provider,
        environment: args.env.clone(),
        worker_url: args.worker_url,
        account_id: None,
    };

    config.save()?;

    eprintln!("Created {}", CONFIG_FILENAME);
    eprintln!("  provider: {}", config.provider);
    eprintln!("  environment: {}", config.environment);
    if let Some(ref url) = config.worker_url {
        eprintln!("  worker_url: {}", url);
    }
    eprintln!();
    eprintln!("Next: otlp2pipeline cf create");

    Ok(())
}
