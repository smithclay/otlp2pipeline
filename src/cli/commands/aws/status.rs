use anyhow::{bail, Result};
use std::process::Command;

use crate::cli::config::Config;
use crate::cli::AwsStatusArgs;

pub async fn execute_status(args: AwsStatusArgs) -> Result<()> {
    let config = Config::load().ok();

    let env_name = args
        .env
        .clone()
        .or_else(|| config.as_ref().map(|c| c.environment.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No environment specified. Either:\n  \
                1. Run `otlp2pipeline init --provider aws --env <name>` first\n  \
                2. Pass --env <name> explicitly"
            )
        })?;

    let region = args
        .region
        .clone()
        .or_else(|| config.as_ref().and_then(|c| c.region.clone()))
        .unwrap_or_else(|| "us-east-1".to_string());

    let stack_name = format!("otlp2pipeline-{}", env_name);

    // Check if AWS CLI is available
    if Command::new("aws").arg("--version").output().is_err() {
        bail!(
            "AWS CLI not found. Install it from https://aws.amazon.com/cli/\n\n\
            Or check status manually:\n  \
            aws cloudformation describe-stacks --stack-name {} --region {}",
            stack_name,
            region
        );
    }

    eprintln!("==> AWS CloudFormation Stack Status");
    eprintln!("    Stack: {}", stack_name);
    eprintln!("    Region: {}", region);
    eprintln!();

    // Get stack status
    eprintln!("==> Stack Status");
    let status = Command::new("aws")
        .args([
            "cloudformation",
            "describe-stacks",
            "--stack-name",
            &stack_name,
            "--region",
            &region,
            "--query",
            "Stacks[0].{Status:StackStatus,Created:CreationTime,Updated:LastUpdatedTime}",
            "--output",
            "table",
        ])
        .output()?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        if stderr.contains("does not exist") {
            eprintln!("    Stack does not exist");
        } else {
            eprintln!("    Error: {}", stderr.trim());
        }
    } else {
        print!("{}", String::from_utf8_lossy(&status.stdout));
    }

    // Get stack resources
    eprintln!("\n==> Stack Resources");
    let resources = Command::new("aws")
        .args([
            "cloudformation",
            "describe-stack-resources",
            "--stack-name",
            &stack_name,
            "--region",
            &region,
            "--query",
            "StackResources[].{Type:ResourceType,Status:ResourceStatus,LogicalId:LogicalResourceId}",
            "--output",
            "table",
        ])
        .output()?;

    if resources.status.success() {
        print!("{}", String::from_utf8_lossy(&resources.stdout));
    }

    Ok(())
}
