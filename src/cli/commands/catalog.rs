use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::cli::{CatalogListArgs, CatalogPartitionArgs};
use crate::cloudflare::{AddPartitionResult, IcebergClient};

/// Tables to query from the Iceberg catalog
const TABLES: &[&str] = &["logs", "traces", "gauge", "sum"];

/// Config values read from wrangler.toml
struct WranglerConfig {
    account_id: String,
    bucket: String,
}

/// Parse wrangler.toml and extract R2_CATALOG_ACCOUNT_ID and R2_CATALOG_BUCKET
fn read_catalog_config(config_path: &str) -> Result<WranglerConfig> {
    let path = Path::new(config_path);
    if !path.exists() {
        bail!(
            "Config file not found: {}\n\n\
            Run this command from a directory with wrangler.toml, or specify --config <path>",
            config_path
        );
    }

    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", config_path))?;

    let config: toml::Value =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", config_path))?;

    let vars = config
        .get("vars")
        .ok_or_else(|| anyhow::anyhow!("Missing [vars] section in {}", config_path))?;

    let account_id = vars
        .get("R2_CATALOG_ACCOUNT_ID")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Missing R2_CATALOG_ACCOUNT_ID in [vars] section of {}\n\n\
                Add this variable to your wrangler.toml:\n  \
                [vars]\n  \
                R2_CATALOG_ACCOUNT_ID = \"your-account-id\"",
                config_path
            )
        })?
        .to_string();

    let bucket = vars
        .get("R2_CATALOG_BUCKET")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Missing R2_CATALOG_BUCKET in [vars] section of {}\n\n\
                Add this variable to your wrangler.toml:\n  \
                [vars]\n  \
                R2_CATALOG_BUCKET = \"your-bucket-name\"",
                config_path
            )
        })?
        .to_string();

    Ok(WranglerConfig { account_id, bucket })
}

pub async fn execute_catalog_list(args: CatalogListArgs) -> Result<()> {
    // Read config from wrangler.toml
    let config = read_catalog_config(&args.config)?;

    eprintln!("==> Querying Iceberg catalog");
    eprintln!("    Account: {}", config.account_id);
    eprintln!("    Bucket: {}", config.bucket);

    let mut client = IcebergClient::new(args.r2_token, config.account_id, config.bucket)?;

    // Fetch catalog config to get the warehouse prefix
    eprint!("    Fetching catalog config... ");
    client.fetch_config().await?;
    eprintln!("ok");
    eprintln!();

    for table in TABLES {
        print_table_info(&client, table).await?;
        println!();
    }

    Ok(())
}

async fn print_table_info(client: &IcebergClient, table: &str) -> Result<()> {
    println!("Table: {}", table);

    match client.get_table_metadata(table).await? {
        Some(metadata) => {
            let inner = &metadata.metadata;

            // UUID
            if let Some(uuid) = &inner.table_uuid {
                println!("  UUID: {}", uuid);
            }

            // Location
            if let Some(location) = &inner.location {
                println!("  Location: {}", location);
            }

            // Current schema ID
            if let Some(schema_id) = inner.current_schema_id {
                println!("  Current schema ID: {}", schema_id);
            }

            // Fields preview
            let fields_preview = inner.field_names_preview(4);
            if !fields_preview.is_empty() {
                println!("  Fields: {}", fields_preview);
            }

            // Partition specs
            let specs = inner.format_partition_specs();
            if !specs.is_empty() {
                println!("  Partition specs:");
                for spec in specs {
                    println!("    {}", spec);
                }
            }

            // Snapshot count
            println!("  Snapshots: {}", inner.snapshots.len());

            // Last updated
            println!("  Last updated: {}", inner.format_last_updated());
        }
        None => {
            println!("  (not found - table may not exist yet)");
        }
    }

    Ok(())
}

pub async fn execute_catalog_partition(args: CatalogPartitionArgs) -> Result<()> {
    // Read config from wrangler.toml
    let config = read_catalog_config(&args.config)?;

    if args.dry_run {
        eprintln!("==> Dry run: would add service_name partition to all tables");
    } else {
        eprintln!("==> Adding service_name partition to all tables");
    }
    eprintln!("    Account: {}", config.account_id);
    eprintln!("    Bucket: {}", config.bucket);

    let mut client = IcebergClient::new(args.r2_token, config.account_id, config.bucket)?;

    // Fetch catalog config to get the warehouse prefix
    eprint!("    Fetching catalog config... ");
    client.fetch_config().await?;
    eprintln!("ok");
    eprintln!();

    let mut success_count = 0;
    let mut skip_count = 0;
    let mut error_count = 0;

    for table in TABLES {
        eprint!("  {} ... ", table);

        if args.dry_run {
            // In dry-run mode, just check the current state
            match client.get_table_metadata(table).await? {
                Some(metadata) => {
                    if metadata.metadata.is_partitioned_by_service_name() {
                        eprintln!("already partitioned by service_name (skip)");
                        skip_count += 1;
                    } else if metadata.metadata.get_service_name_field_id().is_none() {
                        eprintln!("missing service_name field (error)");
                        error_count += 1;
                    } else {
                        eprintln!("would add identity(service_name) partition");
                        success_count += 1;
                    }
                }
                None => {
                    eprintln!("table not found (skip)");
                    skip_count += 1;
                }
            }
        } else {
            // Actually apply the partition spec
            match client.add_partition_spec(table, 1).await {
                Ok(AddPartitionResult::Added) => {
                    eprintln!("added identity(service_name) partition");
                    success_count += 1;
                }
                Ok(AddPartitionResult::AlreadyPartitioned) => {
                    eprintln!("already partitioned by service_name");
                    skip_count += 1;
                }
                Ok(AddPartitionResult::TableNotFound) => {
                    eprintln!("table not found");
                    skip_count += 1;
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    error_count += 1;
                }
            }
        }
    }

    eprintln!();
    if args.dry_run {
        eprintln!(
            "Dry run complete: {} would change, {} skipped, {} errors",
            success_count, skip_count, error_count
        );
    } else {
        eprintln!(
            "Partition evolution complete: {} added, {} skipped, {} errors",
            success_count, skip_count, error_count
        );
    }

    if error_count > 0 {
        bail!("Some tables failed to update");
    }

    Ok(())
}
