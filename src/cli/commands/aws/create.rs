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
use crate::cli::config::{generate_auth_token, Config};
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

    // Generate auth token if requested
    let auth_token = if args.auth {
        Some(generate_auth_token())
    } else {
        None
    };

    let cli = AwsCli::new(&region);
    let mut ctx = DeployContext::new(&cli, &env_name, &args.namespace, args.local)?;
    ctx.auth_token = auth_token.clone();

    eprintln!("==> Deploying otlp2pipeline to AWS");
    eprintln!("    Account:   {}", ctx.account_id);
    eprintln!("    Region:    {}", region);
    eprintln!("    Stack:     {}", stack);
    eprintln!("    Bucket:    {}", ctx.bucket_name);
    eprintln!("    Namespace: {}", ctx.namespace);
    if auth_token.is_some() {
        eprintln!("    Auth:      enabled");
    }

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

    // Phase 6: Configure auth token on Lambda (if --auth and not --local)
    // For --local builds, auth is set during create_function
    if auth_token.is_some() && !args.local {
        eprintln!("\n==> Configuring authentication on Lambda");
        configure_lambda_auth(&cli, &ctx)?;
        eprintln!("    AUTH_TOKEN configured");
    }

    // Save namespace (and auth token) to config
    {
        let mut config = Config::load()?;
        config.namespace = Some(ctx.namespace.clone());
        if let Some(ref token) = auth_token {
            config.auth_token = Some(token.clone());
        }
        config.save()?;
        eprintln!("    Config saved to .otlp2pipeline.toml");
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

/// Configure AUTH_TOKEN on an existing Lambda function
fn configure_lambda_auth(cli: &AwsCli, ctx: &DeployContext) -> Result<()> {
    let function_name = ctx.lambda_function_name();
    let token = ctx
        .auth_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No auth token configured"))?;

    // Build full environment including existing vars
    let env_vars = vec![
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
        ("AUTH_TOKEN".to_string(), token.clone()),
    ];

    cli.lambda()
        .update_function_configuration(&function_name, &env_vars)
}
