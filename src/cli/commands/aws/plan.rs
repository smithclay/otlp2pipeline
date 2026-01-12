use anyhow::Result;

use super::cli::AwsCli;
use super::context::S3_TABLES_ROLE_NAME;
use super::helpers::{load_config, resolve_env_name, resolve_region, stack_name};
use super::schema::TABLES;
use crate::cli::PlanArgs;

pub fn execute_plan(args: PlanArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);
    let stack = stack_name(&env_name);

    let cli = AwsCli::new(&region);
    let account = cli.sts().get_caller_identity()?;

    eprintln!("==> AWS Deployment Plan (Dry Run)\n");
    eprintln!("Account: {}", account.account_id);
    eprintln!("Region:  {}", region);
    eprintln!("Stack:   {}", stack);
    eprintln!();

    // Check current state
    eprintln!("==> Current State\n");

    // IAM Role
    let role_exists = cli.iam().role_exists(S3_TABLES_ROLE_NAME)?;
    eprintln!(
        "IAM Role ({}): {}",
        S3_TABLES_ROLE_NAME,
        if role_exists {
            "exists"
        } else {
            "will be created"
        }
    );

    // Glue catalog
    let catalog_exists = cli.glue().catalog_exists("s3tablescatalog")?;
    eprintln!(
        "Glue Catalog (s3tablescatalog): {}",
        if catalog_exists {
            "exists"
        } else {
            "will be created"
        }
    );

    // Stack
    let stack_exists = cli.cloudformation().describe_stack(&stack)?.is_some();
    eprintln!(
        "CloudFormation Stack ({}): {}",
        stack,
        if stack_exists {
            "exists (will update)"
        } else {
            "will be created"
        }
    );

    eprintln!();
    eprintln!("==> Resources to be Created/Updated\n");

    eprintln!("Phase 0: S3 Tables + LakeFormation");
    eprintln!("  - IAM Role: {}", S3_TABLES_ROLE_NAME);
    eprintln!("  - LakeFormation Admin: {}", account.caller_arn);
    eprintln!(
        "  - LakeFormation Resource: arn:aws:s3tables:{}:{}:bucket/*",
        region, account.account_id
    );
    eprintln!("  - Glue Catalog: s3tablescatalog");
    eprintln!();

    eprintln!("Phase 1: CloudFormation Stack");
    eprintln!("  - Stack: {}", stack);
    eprintln!("  - Table Bucket: {}", stack);
    eprintln!("  - Namespace: default");
    eprintln!(
        "  - Error Bucket: {}-errors-{}-{}",
        stack, account.account_id, region
    );
    eprintln!("  - Firehose IAM Role");
    eprintln!("  - Lambda IAM Role");
    eprintln!("  - CloudWatch Log Group");
    eprintln!();

    eprintln!("Phase 2: Athena Table Creation");
    for table in TABLES {
        eprintln!("  - Table: {} (partitioned by day(timestamp))", table);
    }
    eprintln!();

    eprintln!("Phase 3: LakeFormation Permissions");
    eprintln!("  - DESCRIBE on database 'default'");
    for table in TABLES {
        eprintln!("  - ALL on table '{}'", table);
    }
    eprintln!();

    eprintln!("Phase 4: Firehose Streams");
    for table in TABLES {
        let stream_name = format!("{}-{}", stack, table);
        let exists = cli.firehose().stream_exists(&stream_name)?;
        eprintln!(
            "  - {} (AppendOnly): {}",
            stream_name,
            if exists { "exists" } else { "will be created" }
        );
    }
    eprintln!();

    eprintln!("Phase 5: Lambda (optional --local)");
    let function_name = format!("{}-ingest", stack);
    let function_exists = cli.lambda().function_exists(&function_name)?;
    eprintln!(
        "  - Function: {} {}",
        function_name,
        if function_exists {
            "(exists)"
        } else {
            "(will be created with --local)"
        }
    );
    eprintln!();

    eprintln!("==> To deploy, run:");
    eprintln!(
        "  otlp2pipeline aws create --env {} --region {}",
        env_name, region
    );
    eprintln!();
    eprintln!("To deploy with local Lambda build:");
    eprintln!(
        "  otlp2pipeline aws create --env {} --region {} --local",
        env_name, region
    );

    Ok(())
}
