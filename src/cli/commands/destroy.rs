use anyhow::Result;
use std::io::{self, Write};

use crate::cli::auth;
use crate::cli::DestroyArgs;
use crate::cloudflare::CloudflareClient;

const SIGNAL_NAMES: &[&str] = &["logs", "traces", "gauge", "sum"];

fn bucket_name(env: &str) -> String {
    format!("otlpflare-{}", env.replace('_', "-"))
}

fn stream_name(env: &str, signal: &str) -> String {
    format!("otlpflare_{}_{}", env.replace('-', "_"), signal)
}

fn sink_name(env: &str, signal: &str) -> String {
    format!("otlpflare_{}_{}_sink", env.replace('-', "_"), signal)
}

fn pipeline_name(env: &str, signal: &str) -> String {
    format!("otlpflare_{}_{}", env.replace('-', "_"), signal)
}

pub async fn execute_destroy(args: DestroyArgs) -> Result<()> {
    let bucket = bucket_name(&args.name);

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
                Err(e) => eprintln!("      Failed: {}", e),
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
                Err(e) => eprintln!("      Failed: {}", e),
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
                Err(e) => eprintln!("      Failed: {}", e),
            }
        } else {
            eprintln!("    {}: not found", name);
        }
    }

    // Step 4: Delete bucket
    eprintln!("\n==> Deleting R2 bucket: {}", bucket);
    match client.delete_bucket(&bucket).await {
        Ok(_) => eprintln!("    Deleted"),
        Err(e) => eprintln!("    Failed: {} (may need manual cleanup)", e),
    }

    eprintln!("\n==> Done");

    Ok(())
}
