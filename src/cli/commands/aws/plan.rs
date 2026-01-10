use anyhow::Result;

use super::helpers::{resolve_env_name, stack_name};
use crate::cli::PlanArgs;

pub fn execute_plan(args: PlanArgs) -> Result<()> {
    let env_name = resolve_env_name(args.env)?;
    let stack_name = stack_name(&env_name);

    eprintln!("==> AWS CloudFormation Plan");
    eprintln!();
    eprintln!("Stack name: {}", stack_name);
    eprintln!();
    eprintln!("Resources to be created:");
    eprintln!();
    eprintln!("  S3 Tables:");
    eprintln!("    - TableBucket: otlp2pipeline");
    eprintln!("    - Namespace: default");
    eprintln!("    - Table: logs (Iceberg format)");
    eprintln!();
    eprintln!("  Kinesis Firehose:");
    eprintln!("    - DeliveryStream: {} (DirectPut)", stack_name);
    eprintln!("    - Buffering: 120s / 32MB");
    eprintln!();
    eprintln!("  IAM:");
    eprintln!("    - Role: {}-DeliveryStreamRole-<region>", stack_name);
    eprintln!("    - Policies: GlueAndLakeFormation, LoggingAndErrors");
    eprintln!();
    eprintln!("  Logging:");
    eprintln!("    - LogGroup: /aws/kinesisfirehose/{}", stack_name);
    eprintln!(
        "    - ErrorBucket: {}-firehose-errors-<account>-<region>",
        stack_name
    );
    eprintln!();
    eprintln!("Log table schema (15 fields):");
    eprintln!("  - timestamp (timestamp, required)");
    eprintln!("  - observed_timestamp (long, required)");
    eprintln!("  - trace_id (string)");
    eprintln!("  - span_id (string)");
    eprintln!("  - service_name (string, required)");
    eprintln!("  - service_namespace (string)");
    eprintln!("  - service_instance_id (string)");
    eprintln!("  - severity_number (int, required)");
    eprintln!("  - severity_text (string, required)");
    eprintln!("  - body (string)");
    eprintln!("  - resource_attributes (string/JSON)");
    eprintln!("  - scope_name (string)");
    eprintln!("  - scope_version (string)");
    eprintln!("  - scope_attributes (string/JSON)");
    eprintln!("  - log_attributes (string/JSON)");
    eprintln!();
    eprintln!("To generate template: otlp2pipeline create --output template.yaml");

    Ok(())
}
