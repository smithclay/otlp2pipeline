use anyhow::Result;

use super::helpers::{resolve_env_name, stack_name};
use crate::cli::CreateArgs;

/// Embedded CloudFormation template for OTLP logs
const LOGS_TEMPLATE: &str = include_str!("../../../../templates/aws/logs.cfn.yaml");

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
            std::fs::write(path, LOGS_TEMPLATE)?;
            eprintln!("\n==> Template written to: {}", path);
        }
        None => {
            println!("{}", LOGS_TEMPLATE);
            return Ok(());
        }
    }

    // Print next steps
    let template_file = args.output.as_deref().unwrap_or("template.yaml");

    eprintln!("\n==========================================");
    eprintln!("TEMPLATE GENERATED");
    eprintln!("==========================================\n");
    eprintln!("Next steps:\n");

    eprintln!("1. Deploy Phase 1 (creates S3 Tables, IAM role, logging):");
    eprintln!("   aws cloudformation deploy \\");
    eprintln!("     --template-file {} \\", template_file);
    eprintln!("     --stack-name {} \\", stack_name);
    eprintln!("     --region {} \\", args.region);
    eprintln!("     --capabilities CAPABILITY_NAMED_IAM \\");
    eprintln!(
        "     --parameter-overrides Phase=1 TableBucketName={} NamespaceName={}\n",
        args.table_bucket_name, args.namespace
    );

    eprintln!("2. Grant LakeFormation permissions to the Firehose role:");
    eprintln!(
        "   ./scripts/aws-grant-firehose-permissions.sh {} {} {} {}\n",
        stack_name, args.region, args.table_bucket_name, args.namespace
    );

    eprintln!("3. Deploy Phase 2 (creates Firehose delivery stream):");
    eprintln!("   aws cloudformation deploy \\");
    eprintln!("     --template-file {} \\", template_file);
    eprintln!("     --stack-name {} \\", stack_name);
    eprintln!("     --region {} \\", args.region);
    eprintln!("     --capabilities CAPABILITY_NAMED_IAM \\");
    eprintln!(
        "     --parameter-overrides Phase=2 TableBucketName={} NamespaceName={}\n",
        args.table_bucket_name, args.namespace
    );

    eprintln!("4. Send test data to Firehose:");
    eprintln!(
        "   ./scripts/aws-send-test-record.sh {} {}\n",
        stack_name, args.region
    );

    Ok(())
}
