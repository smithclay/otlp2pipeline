// src/cli/commands/azure/plan.rs
use anyhow::Result;

use super::cli::AzureCli;
use super::context::DeployContext;
use super::helpers::{load_config, resolve_env_name, resolve_region, resolve_resource_group};
use crate::cli::PlanArgs;

pub fn execute_plan(args: PlanArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);
    let resource_group = resolve_resource_group(&None, &env_name);

    let cli = AzureCli::new(&region);
    let ctx = DeployContext::new(&cli, &env_name, &region, Some(resource_group))?;

    eprintln!("==> Azure Deployment Plan\n");
    eprintln!("Subscription:  {}", ctx.subscription_id);
    eprintln!("Region:        {}", region);
    eprintln!();

    eprintln!("Resources to be created:");
    eprintln!();

    eprintln!("Resource Group:");
    eprintln!("  - {}", ctx.resource_group);
    eprintln!();

    eprintln!("Storage Account (ADLS Gen2):");
    eprintln!("  - Name: {}", ctx.storage_account);
    eprintln!("  - Type: StorageV2 with hierarchical namespace");
    eprintln!("  - Containers:");
    for container in &ctx.containers {
        eprintln!("    - {}/", container);
    }
    eprintln!();

    eprintln!("Event Hub:");
    eprintln!("  - Namespace: {}", ctx.eventhub_namespace);
    eprintln!("  - Hub: {}", ctx.eventhub_name);
    eprintln!("  - Partitions: 4");
    eprintln!();

    eprintln!("Stream Analytics Job:");
    eprintln!("  - Name: {}", ctx.stream_analytics_job);
    eprintln!("  - Input: Event Hub (JSON)");
    eprintln!("  - Outputs: 4 Parquet outputs (5 min / 2000 rows batching)");
    eprintln!("    - logs → logs/");
    eprintln!("    - traces → traces/");
    eprintln!("    - gauge → metrics-gauge/");
    eprintln!("    - sum → metrics-sum/");
    eprintln!("  - Query: Route by signal_type field");
    eprintln!();

    eprintln!("Container App:");
    eprintln!("  - Name: {}", ctx.container_app_name);
    eprintln!("  - Environment: otlp-{}-env", env_name);
    eprintln!("  - Image: {} (from ghcr.io)", ctx.container_image);
    eprintln!("  - Resources: 0.5 CPU, 1Gi memory");
    eprintln!("  - Scaling: 1-10 replicas");
    eprintln!("  - Endpoints:");
    eprintln!("    - POST /v1/logs");
    eprintln!("    - POST /v1/traces");
    eprintln!("    - POST /v1/metrics");
    eprintln!();

    eprintln!("To create these resources, run:");
    eprintln!(
        "  otlp2pipeline azure create --env {} --region {}",
        env_name, region
    );

    Ok(())
}
