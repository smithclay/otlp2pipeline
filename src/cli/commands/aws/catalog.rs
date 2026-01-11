use anyhow::{bail, Result};
use std::process::Command;

use super::helpers::{load_config, resolve_env_with_config, resolve_region};
use crate::cli::commands::naming;
use crate::cli::AwsCatalogListArgs;
use crate::cloudflare::TableMetadataInner;

/// Tables to query from S3 Tables
const TABLES: &[&str] = &["logs", "traces", "gauge", "sum"];

pub fn execute_catalog_list(args: AwsCatalogListArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_with_config(args.env, &config)?;
    let region = resolve_region(args.region, &config);

    // Get account_id from config or error
    let account_id = config
        .as_ref()
        .and_then(|c| c.account_id.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "AWS account_id not found in config.\n\n\
                To fix, either:\n  \
                1. Re-run init with AWS CLI configured:\n     \
                   otlp2pipeline init --provider aws --env {} --region {}\n  \
                2. Manually add to .otlp2pipeline.toml:\n     \
                   account_id = \"YOUR_12_DIGIT_ACCOUNT_ID\"",
                env_name,
                region
            )
        })?;

    let bucket_name = format!("otlp2pipeline-{}", naming::normalize(&env_name));
    let table_bucket_arn = format!(
        "arn:aws:s3tables:{}:{}:bucket/{}",
        region, account_id, bucket_name
    );

    eprintln!("==> Querying S3 Tables catalog");
    eprintln!("    Environment: {}", env_name);
    eprintln!("    Region: {}", region);
    eprintln!("    Table Bucket: {}", bucket_name);
    eprintln!();

    // Check for AWS CLI
    if Command::new("aws").arg("--version").output().is_err() {
        bail!(
            "AWS CLI not found. Install it from https://aws.amazon.com/cli/\n\n\
            Or run manually:\n  aws s3tables list-tables --table-bucket-arn {} --region {}",
            table_bucket_arn,
            region
        );
    }

    // List tables in the namespace
    let output = Command::new("aws")
        .args([
            "s3tables",
            "list-tables",
            "--table-bucket-arn",
            &table_bucket_arn,
            "--namespace",
            "default",
            "--region",
            &region,
            "--output",
            "json",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("TableBucketNotFound") || stderr.contains("does not exist") {
            eprintln!("    Table bucket does not exist yet.");
            eprintln!("    Run `otlp2pipeline create` to create the S3 Tables infrastructure.");
            return Ok(());
        }
        bail!("Failed to list tables: {}", stderr.trim());
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let tables_response: serde_json::Value = serde_json::from_str(&json_str)?;

    let tables = tables_response
        .get("tables")
        .and_then(|t| t.as_array())
        .map(|arr| arr.to_vec())
        .unwrap_or_default();

    if tables.is_empty() {
        eprintln!("    No tables found in namespace 'default'.");
        eprintln!("    Tables will be created when data is first ingested.");
        return Ok(());
    }

    // Print table info for each expected table
    for table_name in TABLES {
        print_table_info(&table_bucket_arn, table_name, &region, &tables)?;
        println!();
    }

    // List any unexpected tables
    let expected: std::collections::HashSet<&str> = TABLES.iter().copied().collect();
    let unexpected: Vec<_> = tables
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .filter(|name| !expected.contains(name))
        .collect();

    if !unexpected.is_empty() {
        println!("Other tables:");
        for name in unexpected {
            println!("  - {}", name);
        }
        println!();
    }

    Ok(())
}

fn print_table_info(
    table_bucket_arn: &str,
    table_name: &str,
    region: &str,
    tables: &[serde_json::Value],
) -> Result<()> {
    println!("Table: {}", table_name);

    // Check if table exists in the list
    let table_entry = tables
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(table_name));

    let Some(_entry) = table_entry else {
        println!("  (not found - table may not exist yet)");
        return Ok(());
    };

    // Get detailed table info including metadata location
    let detail_output = Command::new("aws")
        .args([
            "s3tables",
            "get-table",
            "--table-bucket-arn",
            table_bucket_arn,
            "--namespace",
            "default",
            "--name",
            table_name,
            "--region",
            region,
            "--output",
            "json",
        ])
        .output()?;

    if !detail_output.status.success() {
        let stderr = String::from_utf8_lossy(&detail_output.stderr);
        println!("  (error fetching details: {})", stderr.trim());
        return Ok(());
    }

    let detail_json = String::from_utf8_lossy(&detail_output.stdout);
    let detail: serde_json::Value = serde_json::from_str(&detail_json)?;

    // Print UUID if available
    if let Some(uuid) = detail.get("tableARN").and_then(|v| v.as_str()) {
        // Extract table UUID from ARN (last segment after /)
        if let Some(table_uuid) = uuid.rsplit('/').next() {
            println!("  UUID: {}", table_uuid);
        }
    }

    // Print warehouse location
    if let Some(location) = detail.get("warehouseLocation").and_then(|v| v.as_str()) {
        println!("  Location: {}", location);
    }

    // Get metadata location and fetch Iceberg metadata
    if let Some(metadata_location) = detail.get("metadataLocation").and_then(|v| v.as_str()) {
        // Fetch and parse the Iceberg metadata file
        let (metadata_opt, error_opt) = fetch_iceberg_metadata(metadata_location, region);
        if let Some(metadata) = metadata_opt {
            // Current schema ID
            if let Some(schema_id) = metadata.current_schema_id {
                println!("  Current schema ID: {}", schema_id);
            }

            // Fields preview
            let fields_preview = metadata.field_names_preview(4);
            if !fields_preview.is_empty() {
                println!("  Fields: {}", fields_preview);
            }

            // Partition specs
            let specs = metadata.format_partition_specs();
            if !specs.is_empty() {
                println!("  Partition specs:");
                for spec in specs {
                    println!("    {}", spec);
                }
            }

            // Snapshot count
            println!("  Snapshots: {}", metadata.snapshots.len());

            // Last updated
            println!("  Last updated: {}", metadata.format_last_updated());
        } else {
            // Log the specific error if we failed to fetch metadata
            if let Some(error) = error_opt {
                eprintln!("  (could not fetch metadata: {})", error);
            }
            // Fallback: show basic info from get-table response
            if let Some(format) = detail.get("format").and_then(|v| v.as_str()) {
                println!("  Format: {}", format);
            }
            if let Some(created) = detail.get("createdAt").and_then(|v| v.as_str()) {
                println!("  Created: {}", created);
            }
        }
    }

    Ok(())
}

/// Fetch and parse Iceberg metadata from S3
/// Returns (metadata, error_message) - error_message is set if fetch failed
fn fetch_iceberg_metadata(
    metadata_location: &str,
    region: &str,
) -> (Option<TableMetadataInner>, Option<String>) {
    // Use aws s3 cp to fetch the metadata file to stdout
    let output = match Command::new("aws")
        .args([
            "s3",
            "cp",
            metadata_location,
            "-", // output to stdout
            "--region",
            region,
        ])
        .output()
    {
        Ok(out) => out,
        Err(e) => return (None, Some(format!("failed to execute aws s3 cp: {}", e))),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return (None, Some(format!("aws s3 cp failed: {}", stderr.trim())));
    }

    let json_str = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(e) => return (None, Some(format!("metadata not valid UTF-8: {}", e))),
    };

    match serde_json::from_str(&json_str) {
        Ok(metadata) => (Some(metadata), None),
        Err(e) => (
            None,
            Some(format!("failed to parse Iceberg metadata: {}", e)),
        ),
    }
}
