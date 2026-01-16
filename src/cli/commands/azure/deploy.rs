use anyhow::Result;
use std::fs;

use super::cli::{AzureCli, EventHubInputConfig, ParquetOutputConfig};
use super::context::DeployContext;

/// Bicep template for Azure infrastructure (embedded at compile time from external file)
///
/// Single source of truth: templates/azure/otlp.bicep
/// The template is embedded at compile time via include_str!() macro, ensuring the CLI
/// binary works from any directory without needing to locate the templates directory at runtime.
const BICEP_TEMPLATE: &str = include_str!("../../../../templates/azure/otlp.bicep");

/// Stream Analytics query for routing by signal_type
const STREAM_ANALYTICS_QUERY: &str = r#"
-- Route logs by signal_type
SELECT
    *
INTO
    [logsoutput]
FROM
    [eventhubinput]
WHERE
    signal_type = 'logs'

-- Route traces by signal_type
SELECT
    *
INTO
    [tracesoutput]
FROM
    [eventhubinput]
WHERE
    signal_type = 'traces'

-- Route gauge metrics by signal_type
SELECT
    *
INTO
    [gaugeoutput]
FROM
    [eventhubinput]
WHERE
    signal_type = 'metrics_gauge'

-- Route sum metrics by signal_type
SELECT
    *
INTO
    [sumoutput]
FROM
    [eventhubinput]
WHERE
    signal_type = 'metrics_sum'
"#;

/// Deploy Bicep template for storage and Event Hub
pub fn deploy_bicep_template(cli: &AzureCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Phase 1: Deploying Bicep template");

    // Create resource group if not exists
    if !cli.resource().group_exists(&ctx.resource_group)? {
        eprintln!("    Creating resource group: {}", ctx.resource_group);
        cli.resource().create_group(&ctx.resource_group)?;
    } else {
        eprintln!("    Resource group exists: {}", ctx.resource_group);
    }

    // Deploy Bicep template (write embedded template to temp file)
    eprintln!("    Deploying storage account and Event Hub...");

    // Create temp file for the Bicep template
    let temp_dir = std::env::temp_dir();
    let template_path = temp_dir.join(format!("otlp-azure-{}.bicep", ctx.env_name));
    fs::write(&template_path, BICEP_TEMPLATE)?;

    let template_path_str = template_path.to_str().ok_or_else(|| {
        anyhow::anyhow!(
            "Temporary file path contains invalid UTF-8: {:?}. \
                 This may indicate a system configuration issue.",
            template_path
        )
    })?;

    let result = cli.resource().deploy_bicep(
        &ctx.resource_group,
        template_path_str,
        &[
            ("location", &ctx.region),
            ("envName", &ctx.env_name),
            ("storageAccountName", &ctx.storage_account),
            ("eventHubNamespace", &ctx.eventhub_namespace),
        ],
    );

    // Clean up temporary Bicep template
    if let Err(e) = fs::remove_file(&template_path) {
        eprintln!(
            "    Warning: Failed to clean up temporary Bicep template at {}: {}",
            template_path.display(),
            e
        );
        eprintln!(
            "    This file contains infrastructure configuration. Consider removing it manually."
        );
    }

    result?;
    eprintln!("    ✓ Bicep deployment complete");
    Ok(())
}

/// Create and configure Stream Analytics job
pub fn create_stream_analytics_job(cli: &AzureCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Phase 2: Creating Stream Analytics job");

    let sa = cli.stream_analytics();

    // Create job
    if !sa.job_exists(&ctx.stream_analytics_job, &ctx.resource_group)? {
        eprintln!("    Creating job: {}", ctx.stream_analytics_job);
        sa.create_job(&ctx.stream_analytics_job, &ctx.resource_group)?;
    } else {
        eprintln!("    Job exists: {}", ctx.stream_analytics_job);
    }

    // Get connection strings
    eprintln!("    Retrieving connection strings...");
    let eventhub_conn = cli
        .eventhub()
        .get_connection_string(&ctx.eventhub_namespace, &ctx.resource_group)?;
    let storage_conn = cli
        .storage()
        .get_connection_string(&ctx.storage_account, &ctx.resource_group)?;

    // Configure input
    eprintln!("    Configuring Event Hub input...");
    let input_config = EventHubInputConfig::new(
        ctx.eventhub_namespace.clone(),
        ctx.eventhub_name.clone(),
        eventhub_conn,
    )?;
    sa.create_input(
        &ctx.stream_analytics_job,
        &ctx.resource_group,
        &input_config,
    )?;

    // Configure outputs (4 Parquet outputs)
    eprintln!("    Configuring Parquet outputs...");
    let output_names = vec![
        ("logs", "logs"),
        ("traces", "traces"),
        ("gauge", "metrics-gauge"),
        ("sum", "metrics-sum"),
    ];

    for (name, container) in output_names {
        eprintln!("      Creating output: {} → {}/", name, container);
        let output_config = ParquetOutputConfig::new(
            format!("{}output", name),
            ctx.storage_account.clone(),
            container.to_string(),
            storage_conn.clone(),
        )?;
        sa.create_output(
            &ctx.stream_analytics_job,
            &ctx.resource_group,
            &output_config,
        )?;
    }

    // Set query
    eprintln!("    Setting Stream Analytics query...");
    sa.set_query(
        &ctx.stream_analytics_job,
        &ctx.resource_group,
        STREAM_ANALYTICS_QUERY,
    )?;

    eprintln!("    ✓ Stream Analytics job configured");
    Ok(())
}

/// Start Stream Analytics job
pub fn start_stream_analytics_job(cli: &AzureCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Phase 3: Starting Stream Analytics job");

    cli.stream_analytics()
        .start_job(&ctx.stream_analytics_job, &ctx.resource_group)?;

    eprintln!("    ✓ Job started");
    Ok(())
}
