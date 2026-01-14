use anyhow::{Context, Result};

use super::cli::{AzureCli, EventHubInputConfig, ParquetOutputConfig};
use super::context::DeployContext;

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

    // Deploy Bicep template
    eprintln!("    Deploying storage account and Event Hub...");
    let template_path = "templates/azure/otlp.bicep";

    cli.resource().deploy_bicep(
        &ctx.resource_group,
        template_path,
        &[
            ("location", &ctx.region),
            ("envName", &ctx.env_name),
            ("storageAccountName", &ctx.storage_account),
            ("eventHubNamespace", &ctx.eventhub_namespace),
        ],
    )?;

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
    let input_config = EventHubInputConfig {
        namespace: ctx.eventhub_namespace.clone(),
        hub: ctx.eventhub_name.clone(),
        connection_string: eventhub_conn,
    };
    sa.create_input(&ctx.stream_analytics_job, &ctx.resource_group, &input_config)?;

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
        let output_config = ParquetOutputConfig {
            container: container.to_string(),
            storage_account: ctx.storage_account.clone(),
            connection_string: storage_conn.clone(),
        };
        sa.create_output(&ctx.stream_analytics_job, &ctx.resource_group, name, &output_config)?;
    }

    // Set query
    eprintln!("    Setting Stream Analytics query...");
    sa.set_query(&ctx.stream_analytics_job, &ctx.resource_group, STREAM_ANALYTICS_QUERY)?;

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
