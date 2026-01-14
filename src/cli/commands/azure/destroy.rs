// src/cli/commands/azure/destroy.rs
use anyhow::Result;
use std::io::{self, Write};

use super::cli::AzureCli;
use super::context::DeployContext;
use super::helpers::{
    load_config, resolve_env_with_config, resolve_region, resolve_resource_group,
};
use crate::cli::DestroyArgs;

pub fn execute_destroy(args: DestroyArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_with_config(args.env, &config)?;
    let region = resolve_region(args.region, &config);
    let resource_group = resolve_resource_group(&None, &env_name);

    let cli = AzureCli::new(&region);
    let ctx = DeployContext::new(&cli, &env_name, &region, Some(resource_group))?;

    eprintln!("Destroying otlp2pipeline deployment\n");
    eprintln!("Subscription: {}", ctx.subscription_id);
    eprintln!("Region:       {}", region);
    eprintln!("Resource Group: {}", ctx.resource_group);
    eprintln!();

    if !args.force {
        eprintln!("This will delete:");
        eprintln!("  - Stream Analytics job: {}", ctx.stream_analytics_job);
        eprintln!("  - Event Hub namespace: {}", ctx.eventhub_namespace);
        eprintln!(
            "  - Storage account: {} (including all data)",
            ctx.storage_account
        );
        eprintln!("  - Resource group: {}", ctx.resource_group);
        eprintln!();
        eprint!("Are you sure? (yes/no): ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    // Stop and delete Stream Analytics job
    eprintln!("\n==> Stopping Stream Analytics job");
    if cli
        .stream_analytics()
        .job_exists(&ctx.stream_analytics_job, &ctx.resource_group)?
    {
        eprintln!("    Stopping job: {}", ctx.stream_analytics_job);
        let _ = cli
            .stream_analytics()
            .stop_job(&ctx.stream_analytics_job, &ctx.resource_group);

        eprintln!("    Deleting job: {}", ctx.stream_analytics_job);
        cli.stream_analytics()
            .delete_job(&ctx.stream_analytics_job, &ctx.resource_group)?;
    } else {
        eprintln!("    Job does not exist (skipping)");
    }

    // Delete entire resource group (includes all resources)
    eprintln!("\n==> Deleting resource group");
    if cli.resource().group_exists(&ctx.resource_group)? {
        cli.resource().delete_group(&ctx.resource_group)?;
        eprintln!("    ✓ Resource group deletion initiated");
        eprintln!("    Note: Deletion may take several minutes to complete");
    } else {
        eprintln!("    Resource group does not exist (skipping)");
    }

    eprintln!("\n==========================================");
    eprintln!("[ok] Destroy complete");
    eprintln!("==========================================\n");

    Ok(())
}
