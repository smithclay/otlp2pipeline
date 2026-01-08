use anyhow::{bail, Result};
use std::io::{self, Write};

use super::naming::{bucket_name, pipeline_name, sink_name, stream_name, worker_name};
use crate::cli::auth;
use crate::cli::DestroyArgs;
use crate::cloudflare::CloudflareClient;

const SIGNAL_NAMES: &[&str] = &["logs", "traces", "gauge", "sum"];

pub async fn execute_destroy(args: DestroyArgs) -> Result<()> {
    let bucket = bucket_name(&args.name);
    let mut failures: Vec<String> = Vec::new();

    eprintln!("==> Destroying pipeline environment: {}", args.name);
    eprintln!("    Bucket: {}", bucket);

    // Confirmation prompt
    if !args.force {
        eprint!("\nThis will delete all resources. Continue? [y/N] ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    // Resolve auth
    eprintln!("\n==> Resolving credentials...");
    let creds = auth::resolve_credentials()?;
    let client = CloudflareClient::new(creds.token, creds.account_id).await?;
    eprintln!("    Account ID: {}", client.account_id());

    // Step 1: Delete pipelines first (dependency order)
    eprintln!("\n==> Deleting pipelines...");
    let pipelines = client.list_pipelines().await?;
    for signal in SIGNAL_NAMES {
        let name = pipeline_name(&args.name, signal);
        if let Some(pipeline) = pipelines.iter().find(|p| p.name == name) {
            eprintln!("    Deleting: {} ({})", name, pipeline.id);
            match client.delete_pipeline(&pipeline.id).await {
                Ok(_) => eprintln!("      Deleted"),
                Err(e) => {
                    eprintln!("      Failed: {}", e);
                    failures.push(format!("pipeline '{}': {}", name, e));
                }
            }
        } else {
            eprintln!("    {}: not found", name);
        }
    }

    // Step 2: Delete sinks
    eprintln!("\n==> Deleting sinks...");
    let sinks = client.list_sinks().await?;
    for signal in SIGNAL_NAMES {
        let name = sink_name(&args.name, signal);
        if let Some(sink) = sinks.iter().find(|s| s.name == name) {
            eprintln!("    Deleting: {} ({})", name, sink.id);
            match client.delete_sink(&sink.id).await {
                Ok(_) => eprintln!("      Deleted"),
                Err(e) => {
                    eprintln!("      Failed: {}", e);
                    failures.push(format!("sink '{}': {}", name, e));
                }
            }
        } else {
            eprintln!("    {}: not found", name);
        }
    }

    // Step 3: Delete streams
    eprintln!("\n==> Deleting streams...");
    let streams = client.list_streams().await?;
    for signal in SIGNAL_NAMES {
        let name = stream_name(&args.name, signal);
        if let Some(stream) = streams.iter().find(|s| s.name == name) {
            eprintln!("    Deleting: {} ({})", name, stream.id);
            match client.delete_stream(&stream.id).await {
                Ok(_) => eprintln!("      Deleted"),
                Err(e) => {
                    eprintln!("      Failed: {}", e);
                    failures.push(format!("stream '{}': {}", name, e));
                }
            }
        } else {
            eprintln!("    {}: not found", name);
        }
    }

    // Step 4: Delete bucket
    eprintln!("\n==> Deleting R2 bucket: {}", bucket);
    match client.delete_bucket(&bucket).await {
        Ok(_) => eprintln!("    Deleted"),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("not empty") || err_str.contains("BucketNotEmpty") {
                eprintln!("    Failed: bucket is not empty");
                eprintln!();
                eprintln!("    To delete all objects first, run:");
                eprintln!(
                    "      frostbit bucket delete {} --bucket {}",
                    args.name, bucket
                );
                eprintln!();
                eprintln!("    Then re-run destroy to delete the empty bucket.");
                failures.push(format!("bucket '{}': not empty", bucket));
            } else {
                eprintln!("    Failed: {} (may need manual cleanup)", e);
                failures.push(format!("bucket '{}': {}", bucket, e));
            }
        }
    }

    // Step 5: Delete worker (optional)
    if args.include_worker {
        let worker = worker_name(&args.name);
        eprintln!("\n==> Deleting worker: {}", worker);
        match client.delete_worker(&worker).await {
            Ok(_) => eprintln!("    Deleted"),
            Err(e) => {
                eprintln!("    Failed: {}", e);
                failures.push(format!("worker '{}': {}", worker, e));
            }
        }
    }

    // Check if any deletions failed
    if !failures.is_empty() {
        eprintln!(
            "\n==> WARNING: {} resource(s) failed to delete:",
            failures.len()
        );
        for failure in &failures {
            eprintln!("    - {}", failure);
        }
        bail!(
            "Destroy completed with {} failure(s). Manual cleanup may be required.",
            failures.len()
        );
    }

    eprintln!("\n==> Done");

    Ok(())
}
