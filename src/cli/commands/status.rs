use anyhow::Result;

use super::naming::{pipeline_name, sink_name, stream_name};
use crate::cli::auth;
use crate::cli::StatusArgs;
use crate::cloudflare::CloudflareClient;

const SIGNAL_NAMES: &[&str] = &["logs", "traces", "gauge", "sum"];

pub async fn execute_status(args: StatusArgs) -> Result<()> {
    println!("==> Pipeline environment status: {}", args.name);

    // Resolve auth
    let creds = auth::resolve_credentials()?;
    let client = CloudflareClient::new(creds.token, creds.account_id).await?;
    println!("    Account ID: {}", client.account_id());

    // Streams
    println!("\n==> Streams:");
    let streams = client.list_streams().await?;
    for signal in SIGNAL_NAMES {
        let name = stream_name(&args.name, signal);
        if let Some(stream) = streams.iter().find(|s| s.name == name) {
            let endpoint = stream.endpoint.as_deref().unwrap_or("no endpoint");
            println!("    {}: {}", signal, endpoint);
        } else {
            println!("    {}: NOT FOUND", signal);
        }
    }

    // Sinks
    println!("\n==> Sinks:");
    let sinks = client.list_sinks().await?;
    for signal in SIGNAL_NAMES {
        let name = sink_name(&args.name, signal);
        if let Some(sink) = sinks.iter().find(|s| s.name == name) {
            println!("    {}: {} ({})", signal, name, sink.id);
        } else {
            println!("    {}: NOT FOUND", signal);
        }
    }

    // Pipelines
    println!("\n==> Pipelines:");
    let pipelines = client.list_pipelines().await?;
    for signal in SIGNAL_NAMES {
        let name = pipeline_name(&args.name, signal);
        if let Some(pipeline) = pipelines.iter().find(|p| p.name == name) {
            let status = pipeline.status.as_deref().unwrap_or("unknown");
            println!("    {}: {} ({})", signal, name, status);
        } else {
            println!("    {}: NOT FOUND", signal);
        }
    }

    Ok(())
}
