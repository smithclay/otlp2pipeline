// src/cli/commands/azure/create.rs
use anyhow::Result;

use super::cli::AzureCli;
use super::context::DeployContext;
use super::deploy::{
    create_stream_analytics_job, deploy_bicep_template, start_stream_analytics_job,
};
use super::helpers::{load_config, resolve_env_name, resolve_region, resolve_resource_group};
use crate::cli::CreateArgs;

pub fn execute_create(args: CreateArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);
    let resource_group = resolve_resource_group(&None, &env_name);

    let cli = AzureCli::new(&region);
    let ctx = DeployContext::new(&cli, &env_name, &region, Some(resource_group))?;

    eprintln!("==> Deploying otlp2pipeline to Azure");
    eprintln!("    Subscription: {}", ctx.subscription_id);
    eprintln!("    Region:       {}", region);
    eprintln!("    Resource Group: {}", ctx.resource_group);
    eprintln!("    Storage:      {}", ctx.storage_account);
    eprintln!(
        "    Event Hub:    {}/{}",
        ctx.eventhub_namespace, ctx.eventhub_name
    );
    eprintln!("    Stream Analytics: {}", ctx.stream_analytics_job);

    // Phase 1: Deploy Bicep template (storage + Event Hub)
    deploy_bicep_template(&cli, &ctx)?;

    // Phase 2: Create Stream Analytics job
    create_stream_analytics_job(&cli, &ctx)?;

    // Phase 3: Start Stream Analytics job
    start_stream_analytics_job(&cli, &ctx)?;

    // Output connection string for example script
    eprintln!("\n==========================================");
    eprintln!("[ok] Deployment complete!");
    eprintln!("==========================================\n");

    eprintln!("To send events, get the Event Hub connection string:");
    eprintln!("  az eventhubs namespace authorization-rule keys list \\");
    eprintln!("    --resource-group {} \\", ctx.resource_group);
    eprintln!("    --namespace-name {} \\", ctx.eventhub_namespace);
    eprintln!("    --name RootManageSharedAccessKey \\");
    eprintln!("    --query primaryConnectionString -o tsv");
    eprintln!();
    eprintln!("Then run the example:");
    eprintln!("  export EVENTHUB_CONNECTION_STRING=\"<connection-string>\"");
    eprintln!("  cargo run --example azure_eventhub_poc --features azure");

    Ok(())
}
