use anyhow::Result;

use super::naming::{bucket_name, pipeline_name, sink_name, stream_name};
use crate::cli::auth;
use crate::cli::PlanArgs;
use crate::cloudflare::CloudflareClient;

const SIGNAL_NAMES: &[&str] = &["logs", "traces", "gauge", "sum"];
const SIGNAL_SCHEMAS: &[&str] = &[
    "schemas/logs.schema.json",
    "schemas/spans.schema.json",
    "schemas/gauge.schema.json",
    "schemas/sum.schema.json",
];

pub async fn execute_plan(args: PlanArgs) -> Result<()> {
    let bucket = bucket_name(&args.name);

    println!("==> Dry run for environment: {}", args.name);
    println!();
    println!("Would create:");
    println!("  R2 Bucket: {}", bucket);
    println!();
    println!("  Streams:");
    for (i, signal) in SIGNAL_NAMES.iter().enumerate() {
        println!(
            "    - {} (schema: {})",
            stream_name(&args.name, signal),
            SIGNAL_SCHEMAS[i]
        );
    }
    println!();
    println!("  Sinks:");
    for signal in SIGNAL_NAMES {
        println!(
            "    - {} -> table: {}",
            sink_name(&args.name, signal),
            signal
        );
    }
    println!();
    println!("  Pipelines:");
    for signal in SIGNAL_NAMES {
        println!("    - {}", pipeline_name(&args.name, signal));
    }

    println!();
    println!("Checking current state...");
    println!();

    // Resolve auth
    let creds = auth::resolve_credentials()?;
    let client = CloudflareClient::new(creds.token, creds.account_id).await?;

    // Check streams
    let streams = client.list_streams().await?;
    for signal in SIGNAL_NAMES {
        let name = stream_name(&args.name, signal);
        if let Some(stream) = streams.iter().find(|s| s.name == name) {
            println!("  Stream {}: EXISTS ({})", name, stream.id);
        } else {
            println!("  Stream {}: not found", name);
        }
    }

    // Check sinks
    let sinks = client.list_sinks().await?;
    for signal in SIGNAL_NAMES {
        let name = sink_name(&args.name, signal);
        if let Some(sink) = sinks.iter().find(|s| s.name == name) {
            println!("  Sink {}: EXISTS ({})", name, sink.id);
        } else {
            println!("  Sink {}: not found", name);
        }
    }

    // Check pipelines
    let pipelines = client.list_pipelines().await?;
    for signal in SIGNAL_NAMES {
        let name = pipeline_name(&args.name, signal);
        if let Some(pipeline) = pipelines.iter().find(|p| p.name == name) {
            println!("  Pipeline {}: EXISTS ({})", name, pipeline.id);
        } else {
            println!("  Pipeline {}: not found", name);
        }
    }

    Ok(())
}
