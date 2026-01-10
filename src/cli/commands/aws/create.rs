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
    eprintln!("\n==========================================");
    eprintln!("TEMPLATE GENERATED");
    eprintln!("==========================================\n");
    eprintln!("Next steps:\n");
    eprintln!("1. Deploy Phase 1 (creates S3 Tables, IAM role, logging):");
    eprintln!("   aws cloudformation deploy \\");
    eprintln!(
        "     --template-file {} \\",
        args.output.as_deref().unwrap_or("template.yaml")
    );
    eprintln!("     --stack-name {} \\", stack_name);
    eprintln!("     --region {} \\", args.region);
    eprintln!("     --capabilities CAPABILITY_NAMED_IAM \\");
    eprintln!(
        "     --parameter-overrides Phase=1 TableBucketName={} NamespaceName={}\n",
        args.table_bucket_name, args.namespace
    );

    eprintln!("2. Grant LakeFormation permissions to the Firehose role:");
    eprintln!("   - Go to AWS Console > Lake Formation > Data permissions");
    eprintln!(
        "   - Grant the role '{}-DeliveryStreamRole-{}' these permissions:",
        stack_name, args.region
    );
    eprintln!(
        "     - DESCRIBE on s3tablescatalog and s3tablescatalog/{}",
        args.table_bucket_name
    );
    eprintln!("     - ALL (Super) on the logs table");
    eprintln!("   - See: https://docs.aws.amazon.com/AmazonS3/latest/userguide/grant-permissions-tables.html\n");

    eprintln!("3. Deploy Phase 2 (creates Firehose delivery stream):");
    eprintln!("   aws cloudformation deploy \\");
    eprintln!(
        "     --template-file {} \\",
        args.output.as_deref().unwrap_or("template.yaml")
    );
    eprintln!("     --stack-name {} \\", stack_name);
    eprintln!("     --region {} \\", args.region);
    eprintln!("     --capabilities CAPABILITY_NAMED_IAM \\");
    eprintln!(
        "     --parameter-overrides Phase=2 TableBucketName={} NamespaceName={}\n",
        args.table_bucket_name, args.namespace
    );

    eprintln!("4. Send test data to Firehose:");
    eprintln!("   aws firehose put-record-batch \\");
    eprintln!("     --delivery-stream-name {} \\", stack_name);
    eprintln!("     --region {} \\", args.region);
    eprintln!("     --records file://records.json\n");

    eprintln!("   Example records.json:");
    eprintln!(
        r#"   {{"timestamp":"2024-01-01T00:00:00Z","observed_timestamp":1704067200000,"service_name":"my-service","severity_number":9,"severity_text":"INFO","body":"Hello world"}}"#
    );

    Ok(())
}
