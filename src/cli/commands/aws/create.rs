use anyhow::Result;

use super::cli::AwsCli;
use super::context::DeployContext;
use super::deploy::{
    build_and_deploy_lambda, create_firehose_streams, create_tables_via_athena,
    grant_firehose_permissions, setup_s3_tables,
};
use super::helpers::{
    load_config, resolve_env_name, resolve_region, stack_name, validate_name_lengths,
};
use crate::cli::CreateArgs;

/// Embedded CloudFormation template for OTLP signals
const OTLP_TEMPLATE: &str = include_str!("../../../../templates/aws/otlp.cfn.yaml");

pub fn execute_create(args: CreateArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);
    let stack = stack_name(&env_name);

    // Validate name lengths before proceeding
    validate_name_lengths(&stack, &region)?;

    let cli = AwsCli::new(&region);
    let mut ctx = DeployContext::new(&cli, &env_name, &args.namespace, args.local)?;

    eprintln!("==> Deploying otlp2pipeline to AWS");
    eprintln!("    Account:   {}", ctx.account_id);
    eprintln!("    Region:    {}", region);
    eprintln!("    Stack:     {}", stack);
    eprintln!("    Bucket:    {}", ctx.bucket_name);
    eprintln!("    Namespace: {}", ctx.namespace);

    // Phase 0: S3 Tables + LakeFormation setup
    setup_s3_tables(&cli, &ctx)?;

    // Phase 1: Deploy CloudFormation stack
    eprintln!("\n==> Deploying CloudFormation stack");
    let mut params = vec![
        ("TableBucketName", ctx.bucket_name.as_str()),
        ("NamespaceName", ctx.namespace.as_str()),
    ];
    if args.local {
        params.push(("SkipLambda", "true"));
        eprintln!("    (Lambda will be deployed separately from local build)");
    }

    cli.cloudformation()
        .deploy(&stack, OTLP_TEMPLATE, &params)?;
    eprintln!("    CloudFormation complete");

    // Fetch stack outputs
    if let Some(info) = cli.cloudformation().describe_stack(&stack)? {
        ctx.set_stack_outputs(info.outputs);
    }

    // Phase 2: Create tables via Athena DDL
    create_tables_via_athena(&cli, &ctx)?;

    // Phase 3: Grant LakeFormation permissions
    grant_firehose_permissions(&cli, &ctx)?;

    // Phase 4: Create Firehose streams
    create_firehose_streams(&cli, &ctx)?;

    // Phase 5: Local Lambda build (if --local)
    if args.local {
        build_and_deploy_lambda(&cli, &ctx)?;
    }

    // Print success
    eprintln!("\n==========================================");
    eprintln!("Deployment complete!");
    eprintln!("==========================================\n");

    // Print endpoints
    if let Some(url) = cli.lambda().get_function_url(&ctx.lambda_function_name())? {
        eprintln!("OTLP Endpoints:");
        eprintln!("  POST {}v1/logs", url);
        eprintln!("  POST {}v1/traces", url);
        eprintln!("  POST {}v1/metrics", url);
        eprintln!();
    }

    eprintln!("Test with:");
    eprintln!("  ./scripts/aws-send-test-record.sh {} {}", stack, region);
    eprintln!();
    eprintln!("Check status:");
    eprintln!(
        "  otlp2pipeline aws status --env {} --region {}",
        env_name, region
    );

    Ok(())
}
