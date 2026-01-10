use anyhow::{bail, Result};
use std::io::{self, Write};
use std::process::Command;

use crate::cli::config::Config;
use crate::cli::AwsDestroyArgs;

pub async fn execute_destroy(args: AwsDestroyArgs) -> Result<()> {
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
            Or delete manually:\n  \
            aws cloudformation delete-stack --stack-name {} --region {}",
            stack_name,
            region
        );
    }

    eprintln!("==> AWS CloudFormation Stack Deletion");
    eprintln!("    Stack: {}", stack_name);
    eprintln!("    Region: {}", region);
    eprintln!();

    if !args.force {
        eprintln!("WARNING: This will delete:");
        eprintln!("  - S3 Table Bucket and all data");
        eprintln!("  - Firehose delivery stream");
        eprintln!("  - IAM role and policies");
        eprintln!("  - CloudWatch log group");
        eprintln!("  - Error bucket");
        eprintln!();
        eprint!("Are you sure? [y/N] ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    eprintln!("==> Deleting stack...");
    let delete = Command::new("aws")
        .args([
            "cloudformation",
            "delete-stack",
            "--stack-name",
            &stack_name,
            "--region",
            &region,
        ])
        .output()?;

    if !delete.status.success() {
        let stderr = String::from_utf8_lossy(&delete.stderr);
        if stderr.contains("does not exist") {
            eprintln!("    Stack does not exist");
            return Ok(());
        } else {
            bail!("Failed to delete stack: {}", stderr.trim());
        }
    }

    eprintln!("    Delete initiated");
    eprintln!();
    eprintln!("==> Waiting for deletion to complete...");
    eprintln!("    (This may take a few minutes)");

    let wait = Command::new("aws")
        .args([
            "cloudformation",
            "wait",
            "stack-delete-complete",
            "--stack-name",
            &stack_name,
            "--region",
            &region,
        ])
        .status()?;

    if wait.success() {
        eprintln!();
        eprintln!("Stack deleted successfully.");
    } else {
        eprintln!();
        eprintln!("Warning: Stack deletion may have failed or timed out.");
        eprintln!(
            "Check status with: otlp2pipeline aws status --env {}",
            env_name
        );
    }

    Ok(())
}
