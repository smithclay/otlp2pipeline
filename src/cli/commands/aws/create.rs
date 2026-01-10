use anyhow::{Context, Result};

use super::helpers::{load_config, resolve_env_name, resolve_region, stack_name};
use crate::cli::CreateArgs;

/// Embedded CloudFormation template for OTLP signals
const OTLP_TEMPLATE: &str = include_str!("../../../../templates/aws/otlp.cfn.yaml");

pub fn execute_create(args: CreateArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);
    let stack_name = stack_name(&env_name);

    eprintln!("==> Generating CloudFormation template");
    eprintln!("    Environment: {}", env_name);
    eprintln!("    Stack name: {}", stack_name);
    eprintln!("    Region: {}", region);

    // Output to file or stdout
    let Some(path) = &args.output else {
        // No output file specified - print template to stdout and exit
        println!("{}", OTLP_TEMPLATE);
        return Ok(());
    };

    std::fs::write(path, OTLP_TEMPLATE)
        .with_context(|| format!("Failed to write template to '{}'", path))?;
    eprintln!("\n==> Template written to: {}", path);

    // Print next steps
    eprintln!("\n==========================================");
    eprintln!("TEMPLATE GENERATED");
    eprintln!("==========================================\n");
    eprintln!("Next steps:\n");

    eprintln!("1. Deploy infrastructure:");
    eprintln!(
        "   ./scripts/aws-deploy.sh {} --env {} --region {}\n",
        path, env_name, region
    );

    eprintln!("2. Check status:");
    eprintln!(
        "   ./scripts/aws-deploy.sh status --env {} --region {}\n",
        env_name, region
    );

    eprintln!("3. (Optional) Send test data:");
    eprintln!(
        "   ./scripts/aws-send-test-record.sh {} {}\n",
        stack_name, region
    );

    Ok(())
}
