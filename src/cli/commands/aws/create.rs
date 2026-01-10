use anyhow::Result;

use super::helpers::{resolve_env_name, stack_name};
use crate::cli::CreateArgs;

/// Embedded CloudFormation template for OTLP signals
const OTLP_TEMPLATE: &str = include_str!("../../../../templates/aws/otlp.cfn.yaml");

pub fn execute_create(args: CreateArgs) -> Result<()> {
    let env_name = resolve_env_name(args.env)?;
    let stack_name = stack_name(&env_name);

    eprintln!("==> Generating CloudFormation template");
    eprintln!("    Environment: {}", env_name);
    eprintln!("    Stack name: {}", stack_name);
    eprintln!("    Region: {}", args.region);
    eprintln!("    Table bucket: {}", args.table_bucket_name);
    eprintln!("    Namespace: {}", args.namespace);

    match &args.output {
        Some(path) => {
            std::fs::write(path, OTLP_TEMPLATE)?;
            eprintln!("\n==> Template written to: {}", path);
        }
        None => {
            println!("{}", OTLP_TEMPLATE);
            return Ok(());
        }
    }

    // Print next steps
    let template_file = args.output.as_deref().unwrap_or("template.yaml");

    eprintln!("\n==========================================");
    eprintln!("TEMPLATE GENERATED");
    eprintln!("==========================================\n");
    eprintln!("Next steps:\n");

    eprintln!("1. Deploy infrastructure:");
    eprintln!(
        "   ./scripts/aws-deploy.sh {} --env {} --region {}\n",
        template_file, env_name, args.region
    );

    eprintln!("2. Check status:");
    eprintln!(
        "   ./scripts/aws-deploy.sh status --env {} --region {}\n",
        env_name, args.region
    );

    eprintln!("3. (Optional) Send test data:");
    eprintln!(
        "   ./scripts/aws-send-test-record.sh {} {}\n",
        stack_name, args.region
    );

    Ok(())
}
