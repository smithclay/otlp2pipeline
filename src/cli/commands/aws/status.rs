use anyhow::Result;

use super::cli::AwsCli;
use super::context::S3_TABLES_ROLE_NAME;
use super::helpers::{load_config, resolve_env_with_config, resolve_region, stack_name};
use super::schema::TABLES;
use crate::cli::StatusArgs;

pub fn execute_status(args: StatusArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_with_config(args.env, &config)?;
    let region = resolve_region(args.region, &config);
    let stack = stack_name(&env_name);

    let cli = AwsCli::new(&region);
    let account = cli.sts().get_caller_identity()?;

    eprintln!("Checking deployment status...\n");
    eprintln!("Account: {}", account.account_id);
    eprintln!("Region:  {}", region);
    eprintln!("Stack:   {}", stack);
    eprintln!();

    // S3 Tables Setup
    eprintln!("S3 Tables Setup:");

    // IAM Role
    if cli.iam().role_exists(S3_TABLES_ROLE_NAME)? {
        eprintln!("  [ok] IAM Role: {}", S3_TABLES_ROLE_NAME);
    } else {
        eprintln!("  [missing] IAM Role: {} (not found)", S3_TABLES_ROLE_NAME);
    }

    // LakeFormation resource
    let resource_arn = format!(
        "arn:aws:s3tables:{}:{}:bucket/*",
        region, account.account_id
    );
    if cli.lakeformation().describe_resource(&resource_arn)? {
        eprintln!("  [ok] LakeFormation Resource: registered");
    } else {
        eprintln!("  [missing] LakeFormation Resource: not registered");
    }

    // Glue catalog
    if cli.glue().catalog_exists("s3tablescatalog")? {
        eprintln!("  [ok] Glue Catalog: s3tablescatalog");
    } else {
        eprintln!("  [missing] Glue Catalog: s3tablescatalog (not found)");
    }

    eprintln!();

    // CloudFormation Stack
    eprintln!("CloudFormation Stack: {}", stack);
    let stack_info = cli.cloudformation().describe_stack(&stack)?;

    match stack_info {
        Some(info) => {
            eprintln!("  [ok] Status: {}", info.status);

            // Firehose streams
            eprintln!();
            eprintln!("Firehose Streams:");
            let mut all_ready = true;
            for table in TABLES {
                let stream_name = format!("{}-{}", stack, table);
                if cli.firehose().stream_exists(&stream_name)? {
                    eprintln!("  [ok] {} (AppendOnly: true)", stream_name);
                } else {
                    eprintln!("  [missing] {} (not found)", stream_name);
                    all_ready = false;
                }
            }

            // Lambda function
            eprintln!();
            eprintln!("Lambda Function:");
            let function_name = format!("{}-ingest", stack);
            if cli.lambda().function_exists(&function_name)? {
                eprintln!("  [ok] {}", function_name);
                if let Some(url) = cli.lambda().get_function_url(&function_name)? {
                    eprintln!();
                    eprintln!("OTLP Endpoints:");
                    eprintln!("  POST {}v1/logs", url);
                    eprintln!("  POST {}v1/traces", url);
                    eprintln!("  POST {}v1/metrics", url);
                }
            } else {
                eprintln!("  [missing] {} (not found)", function_name);
            }

            eprintln!();
            if all_ready {
                eprintln!("[ok] Deployment complete! Firehose is ready to receive data.");
            } else {
                eprintln!("[warn] Some Firehose streams are missing. Run deploy to create them.");
            }
        }
        None => {
            eprintln!("  [missing] Stack does not exist");
        }
    }

    Ok(())
}
