use anyhow::Result;

use super::cli::AzureCli;
use super::context::DeployContext;
use super::helpers::{
    load_config, resolve_env_with_config, resolve_region, resolve_resource_group,
};
use crate::cli::StatusArgs;

pub fn execute_status(args: StatusArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_with_config(args.env, &config)?;
    let region = resolve_region(args.region, &config);
    let resource_group = resolve_resource_group(&None, &env_name);

    let cli = AzureCli::new(&region);
    let ctx = DeployContext::new(&cli, &env_name, &region, Some(resource_group))?;

    eprintln!("Checking deployment status...\n");
    eprintln!("Subscription: {}", ctx.subscription_id);
    eprintln!("Region:       {}", region);
    eprintln!("Resource Group: {}", ctx.resource_group);
    eprintln!();

    // Check resource group
    eprintln!("Resource Group:");
    if cli.resource().group_exists(&ctx.resource_group)? {
        eprintln!("  [ok] {}", ctx.resource_group);
    } else {
        eprintln!("  [missing] {} (not found)", ctx.resource_group);
        eprintln!("\nRun: otlp2pipeline azure create --env {}", env_name);
        return Ok(());
    }

    // Check storage account
    eprintln!();
    eprintln!("Storage Account:");
    if cli
        .storage()
        .account_exists(&ctx.storage_account, &ctx.resource_group)?
    {
        eprintln!("  [ok] {} (ADLS Gen2)", ctx.storage_account);

        // Check containers
        eprintln!("    Containers:");
        for container in &ctx.containers {
            if cli
                .storage()
                .container_exists(container, &ctx.storage_account)?
            {
                eprintln!("      [ok] {}/", container);
            } else {
                eprintln!("      [missing] {}/", container);
            }
        }
    } else {
        eprintln!("  [missing] {} (not found)", ctx.storage_account);
    }

    // Check Event Hub
    eprintln!();
    eprintln!("Event Hub:");
    if cli
        .eventhub()
        .namespace_exists(&ctx.eventhub_namespace, &ctx.resource_group)?
    {
        eprintln!("  [ok] Namespace: {}", ctx.eventhub_namespace);

        if cli.eventhub().hub_exists(
            &ctx.eventhub_namespace,
            &ctx.eventhub_name,
            &ctx.resource_group,
        )? {
            eprintln!("  [ok] Hub: {}", ctx.eventhub_name);
        } else {
            eprintln!("  [missing] Hub: {}", ctx.eventhub_name);
        }
    } else {
        eprintln!("  [missing] Namespace: {}", ctx.eventhub_namespace);
    }

    // Check Stream Analytics job
    eprintln!();
    eprintln!("Stream Analytics Job:");
    if cli
        .stream_analytics()
        .job_exists(&ctx.stream_analytics_job, &ctx.resource_group)?
    {
        let state = cli
            .stream_analytics()
            .get_job_state(&ctx.stream_analytics_job, &ctx.resource_group)?;
        eprintln!("  [ok] Job: {}", ctx.stream_analytics_job);
        eprintln!("  [ok] State: {}", state);
    } else {
        eprintln!("  [missing] Job: {}", ctx.stream_analytics_job);
    }

    // Check Container App
    eprintln!();
    eprintln!("Container App:");
    if cli
        .containerapp()
        .exists(&ctx.container_app_name, &ctx.resource_group)?
    {
        let state = cli
            .containerapp()
            .get_state(&ctx.container_app_name, &ctx.resource_group)?;
        let url = cli
            .containerapp()
            .get_url(&ctx.container_app_name, &ctx.resource_group)?;
        eprintln!("  [ok] {}", ctx.container_app_name);
        eprintln!("  [ok] State: {}", state);
        eprintln!("  [ok] URL: {}", url);
        eprintln!("  [ok] Image: {}", ctx.container_image);
    } else {
        eprintln!("  [missing] {}", ctx.container_app_name);
    }

    eprintln!();
    eprintln!("[ok] Status check complete");

    Ok(())
}
