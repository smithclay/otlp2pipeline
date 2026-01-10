use anyhow::{bail, Result};
use std::io::{self, Write};
use std::process::Command;

use super::helpers::{load_config, require_aws_cli, resolve_region, stack_name};
use crate::cli::DestroyArgs;

pub fn execute_destroy(args: DestroyArgs) -> Result<()> {
    let config = load_config()?;

    let env_name = args
        .env
        .or_else(|| config.as_ref().map(|c| c.environment.clone()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No environment specified. Either:\n  \
                1. Run `otlp2pipeline init --provider aws --env <name>` first\n  \
                2. Pass --env <name> explicitly"
            )
        })?;

    let region = resolve_region(args.region, &config);
    let stack_name = stack_name(&env_name);

    require_aws_cli(&stack_name, &region, "delete-stack")?;

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
        Ok(())
    } else {
        eprintln!();
        eprintln!("Stack deletion may have failed or timed out.");
        eprintln!(
            "Check status with: otlp2pipeline aws status --env {}",
            env_name
        );
        bail!("Stack deletion did not complete successfully")
    }
}
