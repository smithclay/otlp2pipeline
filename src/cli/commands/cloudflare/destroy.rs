use anyhow::{bail, Result};
use std::io::{self, Write};

use crate::cli::auth;
use crate::cli::commands::naming::{
    access_app_name, bucket_name, pipeline_name, sink_name, stream_name, worker_name,
};
use crate::cli::config::Config;
use crate::cli::DestroyArgs;
use crate::cloudflare::CloudflareClient;

const SIGNAL_NAMES: &[&str] = &["logs", "traces", "gauge", "sum"];

pub async fn execute_destroy(args: DestroyArgs) -> Result<()> {
    let env_name = args
        .env
        .clone()
        .or_else(|| Config::load().ok().map(|c| c.environment))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No environment specified. Either:\n  \
        1. Run `otlp2pipeline init --provider cf --env <name>` first\n  \
        2. Pass --env <name> explicitly"
            )
        })?;

    let bucket = bucket_name(&env_name);
    let mut failures: Vec<String> = Vec::new();

    eprintln!("==> Destroying pipeline environment: {}", env_name);
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
        let name = pipeline_name(&env_name, signal);
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
        let name = sink_name(&env_name, signal);
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
        let name = stream_name(&env_name, signal);
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

    // Step 4: Delete Access app if it exists
    eprintln!("\n==> Checking for Access application...");
    let apps = client.list_access_apps().await?;
    let app_name = access_app_name(&env_name);
    if let Some(app) = apps.iter().find(|a| a.name == app_name) {
        eprintln!("    Deleting: {}", app_name);
        match client.delete_access_app(&app.id).await {
            Ok(_) => eprintln!("      Deleted"),
            Err(e) => {
                eprintln!("      Failed: {}", e);
                failures.push(format!("access app '{}': {}", app_name, e));
            }
        }
    } else {
        eprintln!("    No Access app found");
    }

    // Step 5: Delete bucket
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
                    "      otlp2pipeline bucket delete {} --bucket {}",
                    env_name, bucket
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
        let worker = worker_name(&env_name);
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
