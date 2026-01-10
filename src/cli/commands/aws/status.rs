use anyhow::{bail, Result};
use std::process::Command;

use super::helpers::{
    load_config, require_aws_cli, resolve_env_with_config, resolve_region, stack_name,
};
use crate::cli::StatusArgs;

pub fn execute_status(args: StatusArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_with_config(args.env, &config)?;
    let region = resolve_region(args.region, &config);
    let stack_name = stack_name(&env_name);

    require_aws_cli(&stack_name, &region, "describe-stacks")?;

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
            return Ok(());
        } else {
            bail!("Failed to get stack status: {}", stderr.trim());
        }
    }

    print!("{}", String::from_utf8_lossy(&status.stdout));

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
    } else {
        let stderr = String::from_utf8_lossy(&resources.stderr);
        eprintln!("    Failed to retrieve resources: {}", stderr.trim());
    }

    Ok(())
}
