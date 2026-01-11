// src/cli/commands/aws/deploy.rs
use super::cli::{AwsCli, FirehoseStreamConfig, LambdaConfig, QueryState};
use super::context::{
    s3_tables_data_policy, s3_tables_trust_policy, DeployContext, S3_TABLES_ROLE_NAME,
};
use super::schema::{Schema, TABLES};
use anyhow::{bail, Result};
use std::thread;
use std::time::Duration;

/// Phase 0: S3 Tables + LakeFormation setup
pub fn setup_s3_tables(cli: &AwsCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Phase 0: S3 Tables + Lake Formation Setup");

    // Step 1: IAM Role
    eprintln!("\n    Creating/updating IAM role: {}", S3_TABLES_ROLE_NAME);
    let iam = cli.iam();
    if iam.role_exists(S3_TABLES_ROLE_NAME)? {
        eprintln!("    Role exists, updating policies...");
        iam.update_assume_role_policy(S3_TABLES_ROLE_NAME, &s3_tables_trust_policy())?;
    } else {
        eprintln!("    Creating role...");
        iam.create_role(S3_TABLES_ROLE_NAME, &s3_tables_trust_policy())?;
        eprintln!("    Waiting for IAM propagation...");
        thread::sleep(Duration::from_secs(10));
    }
    iam.put_role_policy(
        S3_TABLES_ROLE_NAME,
        "S3TablesDataAccess",
        &s3_tables_data_policy(),
    )?;
    eprintln!("    Done");

    // Step 2: LakeFormation admin
    eprintln!("\n    Adding caller as LakeFormation admin");
    cli.lakeformation()
        .put_data_lake_settings(&[&ctx.caller_arn])?;
    eprintln!("    Done");

    // Step 3: Register resource
    eprintln!("\n    Registering S3 Tables resource with LakeFormation");
    let lf = cli.lakeformation();
    lf.deregister_resource(&ctx.s3_tables_resource_arn())?;
    lf.register_resource(&ctx.s3_tables_resource_arn(), &ctx.s3_tables_role_arn())?;
    eprintln!("    Done");

    // Step 4: Glue catalog
    eprintln!("\n    Creating/updating s3tablescatalog federated catalog");
    let glue = cli.glue();
    // Note: Skip delete - create_catalog handles AlreadyExistsException.
    // Deleting requires DROP permission which we haven't granted yet.
    if glue.create_catalog("s3tablescatalog", &ctx.s3_tables_resource_arn())? {
        eprintln!("    Created");
    } else {
        eprintln!("    Already exists");
    }

    // Step 5: Catalog permissions
    eprintln!("\n    Granting catalog permissions to caller");
    let catalog_resource = serde_json::json!({
        "Catalog": {"Id": format!("{}:s3tablescatalog", ctx.account_id)}
    });
    cli.lakeformation().grant_permissions(
        &ctx.caller_arn,
        &catalog_resource,
        &["ALL", "DESCRIBE", "CREATE_DATABASE", "ALTER", "DROP"],
        true,
    )?;
    eprintln!("    Done");

    Ok(())
}

/// Create tables via Athena DDL (with partition specs)
pub fn create_tables_via_athena(cli: &AwsCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Creating tables via Athena DDL (with partitions)");

    // Grant CREATE_TABLE permission on the database
    eprintln!(
        "\n    Granting CREATE_TABLE permission on database '{}'",
        ctx.namespace
    );
    let db_resource = serde_json::json!({
        "Database": {
            "CatalogId": format!("{}:s3tablescatalog/{}", ctx.account_id, ctx.bucket_name),
            "Name": ctx.namespace
        }
    });
    cli.lakeformation().grant_permissions(
        &ctx.caller_arn,
        &db_resource,
        &["CREATE_TABLE", "DESCRIBE", "ALTER", "DROP"],
        true,
    )?;
    eprintln!("    Done");

    let athena = cli.athena();
    let catalog = format!("s3tablescatalog/{}", ctx.bucket_name);
    let output_location = format!("s3://{}/athena/", ctx.error_bucket_name());

    for table in TABLES {
        eprintln!("\n    Creating table: {}", table);

        let schema = Schema::load(table)?;
        let ddl = schema.to_create_table_ddl(&ctx.namespace, table);

        match athena.execute_query(&ddl, &catalog, &output_location)? {
            QueryState::Succeeded => {
                eprintln!("    Created with day(timestamp) partition");
            }
            QueryState::Failed(reason) => {
                if reason.contains("already exists") {
                    eprintln!("    Table already exists");
                } else {
                    bail!("Failed to create table {}: {}", table, reason);
                }
            }
            other => bail!("Unexpected query state for {}: {:?}", table, other),
        }
    }

    eprintln!("\n    All tables created with partitions");
    Ok(())
}

/// Grant LakeFormation permissions to Firehose role
pub fn grant_firehose_permissions(cli: &AwsCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Granting LakeFormation permissions to Firehose role");

    let firehose_role_arn = ctx
        .get_output("FirehoseRoleARN")
        .ok_or_else(|| anyhow::anyhow!("Missing stack output: FirehoseRoleARN"))?
        .clone();

    eprintln!("    Firehose role: {}", firehose_role_arn);

    let lf = cli.lakeformation();

    // Database permission
    eprintln!("\n    Granting DESCRIBE on database '{}'", ctx.namespace);
    let db_resource = serde_json::json!({
        "Database": {
            "CatalogId": format!("{}:s3tablescatalog/{}", ctx.account_id, ctx.bucket_name),
            "Name": ctx.namespace
        }
    });
    lf.grant_permissions(&firehose_role_arn, &db_resource, &["DESCRIBE"], false)?;
    eprintln!("    Done");

    // Table permissions
    for table in TABLES {
        eprintln!("\n    Granting ALL on table '{}'", table);
        let table_resource = serde_json::json!({
            "Table": {
                "CatalogId": format!("{}:s3tablescatalog/{}", ctx.account_id, ctx.bucket_name),
                "DatabaseName": ctx.namespace,
                "Name": table
            }
        });
        lf.grant_permissions(&firehose_role_arn, &table_resource, &["ALL"], false)?;
        eprintln!("    Done");
    }

    Ok(())
}

/// Create Firehose streams via API (AppendOnly mode)
pub fn create_firehose_streams(cli: &AwsCli, ctx: &DeployContext) -> Result<()> {
    eprintln!("\n==> Creating Firehose streams via API (AppendOnly mode)");

    let firehose_role_arn = ctx
        .get_output("FirehoseRoleARN")
        .ok_or_else(|| anyhow::anyhow!("Missing stack output: FirehoseRoleARN"))?
        .clone();

    let log_group = ctx
        .get_output("FirehoseLogGroupName")
        .ok_or_else(|| anyhow::anyhow!("Missing stack output: FirehoseLogGroupName"))?
        .clone();

    let error_bucket = ctx
        .get_output("FirehoseErrorBucketName")
        .ok_or_else(|| anyhow::anyhow!("Missing stack output: FirehoseErrorBucketName"))?
        .clone();

    let error_prefix = ctx
        .get_output("FirehoseErrorPrefix")
        .unwrap_or(&"errors/".to_string())
        .clone();

    let batch_time: u32 = ctx
        .get_output("FirehoseBatchTime")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);

    let batch_size: u32 = ctx
        .get_output("FirehoseBatchSize")
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);

    eprintln!("    Role ARN: {}", firehose_role_arn);
    eprintln!("    Catalog ARN: {}", ctx.glue_catalog_arn());

    let log_streams = [
        "Logs_Destination_Errors",
        "Traces_Destination_Errors",
        "Sum_Destination_Errors",
        "Gauge_Destination_Errors",
    ];

    let firehose = cli.firehose();

    for (i, table) in TABLES.iter().enumerate() {
        let stream_name = ctx.firehose_stream_name(table);
        eprintln!("\n    Checking stream: {}", stream_name);

        let config = FirehoseStreamConfig {
            name: stream_name.clone(),
            role_arn: firehose_role_arn.clone(),
            catalog_arn: ctx.glue_catalog_arn(),
            database: ctx.namespace.clone(),
            table: table.to_string(),
            log_group: log_group.clone(),
            log_stream: log_streams[i].to_string(),
            error_bucket: error_bucket.clone(),
            error_prefix: error_prefix.clone(),
            batch_interval_secs: batch_time,
            batch_size_mb: batch_size,
        };

        if firehose.create_delivery_stream(&config)? {
            eprintln!("    Created");
        } else {
            eprintln!("    Stream exists (skipping)");
        }
    }

    eprintln!("\n    Firehose streams ready");
    Ok(())
}

/// Build and deploy Lambda from local repo
pub fn build_and_deploy_lambda(cli: &AwsCli, ctx: &DeployContext) -> Result<()> {
    use std::process::Command;

    eprintln!("\n==> Building and deploying Lambda from local repo");

    // Check for cargo-lambda
    if Command::new("cargo-lambda")
        .arg("--version")
        .output()
        .is_err()
    {
        bail!("cargo-lambda not found. Install with: pip3 install cargo-lambda");
    }

    // Build Lambda
    eprintln!("\n    Building Lambda (ARM64)");
    let build_status = Command::new("cargo")
        .args([
            "lambda",
            "build",
            "--release",
            "--arm64",
            "--features",
            "lambda",
            "--bin",
            "lambda",
        ])
        .status()?;

    if !build_status.success() {
        bail!("Lambda build failed");
    }
    eprintln!("    Build complete");

    // Zip the bootstrap binary
    eprintln!("\n    Uploading to S3");
    let build_dir = "target/lambda/lambda";
    let zip_path = format!("/tmp/lambda-{}.zip", ctx.stack_name);

    let zip_status = Command::new("zip")
        .args(["-j", &zip_path, &format!("{}/bootstrap", build_dir)])
        .output()?;

    if !zip_status.status.success() {
        bail!("Failed to create zip file");
    }

    let artifact_bucket = ctx.artifact_bucket_name();
    let s3_key = "lambda/local/bootstrap.zip";

    cli.s3()
        .cp(&zip_path, &format!("s3://{}/{}", artifact_bucket, s3_key))?;
    eprintln!("    Uploaded to s3://{}/{}", artifact_bucket, s3_key);

    // Create or update Lambda function
    eprintln!("\n    Creating/updating Lambda function");
    let function_name = ctx.lambda_function_name();
    let lambda = cli.lambda();

    if lambda.function_exists(&function_name)? {
        lambda.update_function_code(&function_name, &artifact_bucket, s3_key)?;
        eprintln!("    Updated function: {}", function_name);
    } else {
        let config = LambdaConfig {
            name: function_name.clone(),
            role_arn: ctx.lambda_role_arn(),
            s3_bucket: artifact_bucket.clone(),
            s3_key: s3_key.to_string(),
            memory_size: 512,
            timeout: 30,
            environment: vec![
                ("RUST_LOG".to_string(), "info".to_string()),
                (
                    "PIPELINE_LOGS".to_string(),
                    ctx.firehose_stream_name("logs"),
                ),
                (
                    "PIPELINE_TRACES".to_string(),
                    ctx.firehose_stream_name("traces"),
                ),
                ("PIPELINE_SUM".to_string(), ctx.firehose_stream_name("sum")),
                (
                    "PIPELINE_GAUGE".to_string(),
                    ctx.firehose_stream_name("gauge"),
                ),
            ],
        };

        lambda.create_function(&config)?;
        eprintln!("    Created function: {}", function_name);

        // Create function URL
        eprintln!("\n    Creating function URL");
        lambda.create_function_url(&function_name)?;
        lambda.add_public_url_permission(&function_name)?;
        eprintln!("    Function URL created");
    }

    // Get and display function URL
    if let Some(url) = lambda.get_function_url(&function_name)? {
        eprintln!("\n    Function URL: {}", url);
    }

    std::fs::remove_file(&zip_path).ok();
    eprintln!("\n    Lambda deployed from local build");
    Ok(())
}
