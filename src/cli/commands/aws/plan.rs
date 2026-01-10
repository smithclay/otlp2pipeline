use anyhow::Result;

use crate::cli::config::Config;
use crate::cli::AwsPlanArgs;

pub async fn execute_plan(args: AwsPlanArgs) -> Result<()> {
    let env_name = args
        .env
        .clone()
        .or_else(|| Config::load().ok().map(|c| c.environment))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No environment specified. Either:\n  \
                1. Run `otlp2pipeline init --provider aws --env <name>` first\n  \
                2. Pass --env <name> explicitly"
            )
        })?;

    let stack_name = format!("otlp2pipeline-{}", env_name);

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
