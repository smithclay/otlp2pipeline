// src/cli/commands/azure/create.rs
use anyhow::Result;

use super::cli::AzureCli;
use super::context::DeployContext;
use super::deploy::{
    create_stream_analytics_job, deploy_bicep_template, start_stream_analytics_job,
};
use super::helpers::{load_config, resolve_env_name, resolve_region, resolve_resource_group};
use crate::cli::config::{generate_auth_token, Config};
use crate::cli::CreateArgs;

pub fn execute_create(args: CreateArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);
    let resource_group = resolve_resource_group(&None, &env_name);

    // Generate auth token if requested
    let auth_token = if args.auth {
        Some(generate_auth_token())
    } else {
        None
    };

    let cli = AzureCli::new(&region);
    let mut ctx = DeployContext::new(&cli, &env_name, &region, Some(resource_group))?;
    ctx.auth_token = auth_token.clone();

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
    eprintln!("    Container App: {}", ctx.container_app_name);
    if auth_token.is_some() {
        eprintln!("    Auth:         enabled");
    }

    // Phase 1: Deploy Bicep template (storage + Event Hub + Container App)
    deploy_bicep_template(&cli, &ctx)?;

    // Phase 1.5: Configure auth token on Container App (if --auth)
    if auth_token.is_some() {
        eprintln!("\n==> Configuring authentication on Container App");
        configure_container_auth(&cli, &ctx)?;
        eprintln!("    AUTH_TOKEN configured");
    }

    // Phase 2: Create Stream Analytics job
    create_stream_analytics_job(&cli, &ctx)?;

    // Phase 4: Start Stream Analytics job
    start_stream_analytics_job(&cli, &ctx)?;

    // Save config (auth token if enabled)
    {
        let mut config = Config::load()?;
        if let Some(ref token) = auth_token {
            config.auth_token = Some(token.clone());
        }
        config.save()?;
        eprintln!("    Config saved to .otlp2pipeline.toml");
    }

    // Output connection string for example script
    eprintln!("\n==========================================");
    eprintln!("[ok] Deployment complete!");
    eprintln!("==========================================\n");

    // Get Container App URL
    let container_url = cli
        .containerapp()
        .get_url(&ctx.container_app_name, &ctx.resource_group)
        .unwrap_or_else(|_| "unknown".to_string());

    eprintln!("OTLP Endpoints:");
    eprintln!("  POST {}/v1/logs", container_url);
    eprintln!("  POST {}/v1/traces", container_url);
    eprintln!("  POST {}/v1/metrics", container_url);
    eprintln!();

    // Print auth token if generated
    if let Some(ref token) = auth_token {
        eprintln!("Authentication:");
        eprintln!("  Token: {}", token);
        eprintln!("  Header: Authorization: Bearer {}", token);
        eprintln!();
        eprintln!("  IMPORTANT: Keep this token secure. Do not commit it to version control");
        eprintln!("  or share it in logs. The token is saved to .otlp2pipeline.toml and will");
        eprintln!("  be included automatically when using 'otlp2pipeline connect'.");
        eprintln!();
    }

    eprintln!("Check status:");
    eprintln!("  otlp2pipeline azure status --env {}", env_name);

    Ok(())
}

/// Configure AUTH_TOKEN on Container App
fn configure_container_auth(cli: &AzureCli, ctx: &DeployContext) -> Result<()> {
    let token = ctx
        .auth_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No auth token configured"))?;

    cli.containerapp().update_environment_variables(
        &ctx.container_app_name,
        &ctx.resource_group,
        &[("AUTH_TOKEN", token)],
    )
}
