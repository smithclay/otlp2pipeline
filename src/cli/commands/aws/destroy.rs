use anyhow::Result;
use std::io::{self, Write};

use super::cli::AwsCli;
use super::helpers::{load_config, resolve_env_with_config, resolve_region, stack_name};
use super::schema::TABLES;
use crate::cli::DestroyArgs;

pub fn execute_destroy(args: DestroyArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_with_config(args.env, &config)?;
    let region = resolve_region(args.region, &config);
    let stack = stack_name(&env_name);
    // Read namespace from config, defaulting to "default" for backwards compatibility
    let namespace = config
        .as_ref()
        .and_then(|c| c.namespace.clone())
        .unwrap_or_else(|| "default".to_string());

    let cli = AwsCli::new(&region);
    let account = cli.sts().get_caller_identity()?;

    eprintln!("Destroying otlp2pipeline deployment\n");
    eprintln!("Account: {}", account.account_id);
    eprintln!("Region:  {}", region);
    eprintln!("Stack:   {}", stack);
    eprintln!();

    if !args.force {
        eprintln!("This will delete:");
        eprintln!("  - Firehose streams: {}-{{logs,traces,sum,gauge}}", stack);
        eprintln!("  - CloudFormation stack: {}", stack);
        eprintln!("  - S3 Table Bucket: {}", stack);
        eprintln!("  - All data in the bucket");
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

    // Delete Firehose streams first (they depend on IAM role in stack)
    eprintln!("\n==> Deleting Firehose streams");
    let firehose = cli.firehose();
    for table in TABLES {
        let stream_name = format!("{}-{}", stack, table);
        eprintln!("    Deleting stream: {}", stream_name);
        firehose.delete_delivery_stream(&stream_name)?;
    }

    // Delete tables from namespace
    eprintln!("\n==> Deleting tables from namespace");
    let bucket_arn = format!(
        "arn:aws:s3tables:{}:{}:bucket/{}",
        region, account.account_id, stack
    );
    let s3tables = cli.s3tables();
    for table in TABLES {
        eprintln!("    Deleting table: {}", table);
        s3tables.delete_table(&bucket_arn, &namespace, table)?;
    }

    // Empty S3 buckets
    eprintln!("\n==> Emptying S3 buckets");
    let s3 = cli.s3();

    let error_bucket = format!("{}-errors-{}-{}", stack, account.account_id, region);
    eprintln!("    Emptying error bucket: {}", error_bucket);
    s3.rm_recursive(&error_bucket)?;

    let artifact_bucket = format!("{}-artifacts-{}", stack, account.account_id);
    eprintln!("    Emptying artifact bucket: {}", artifact_bucket);
    s3.rm_recursive(&artifact_bucket)?;

    // Delete CloudFormation stack
    if cli.cloudformation().describe_stack(&stack)?.is_some() {
        eprintln!("\n==> Deleting CloudFormation stack: {}", stack);
        cli.cloudformation().delete_stack(&stack)?;
        eprintln!("    Waiting for stack deletion...");
        cli.cloudformation().wait_stack_delete_complete(&stack)?;
        eprintln!("    Stack deleted");
    } else {
        eprintln!("\n    Stack does not exist (skipping)");
    }

    eprintln!("\n==========================================");
    eprintln!("[ok] Destroy complete");
    eprintln!("==========================================\n");
    eprintln!("Note: The following global resources were NOT deleted:");
    eprintln!("  - IAM Role: S3TablesRoleForLakeFormation");
    eprintln!("  - Glue Catalog: s3tablescatalog");
    eprintln!("  - LakeFormation configuration");
    eprintln!();
    eprintln!("These are shared across stacks. Delete manually if no longer needed.");

    Ok(())
}
