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

    // Query Lambda function details
    let lambda_name = format!("{}-ingest", stack_name);
    let lambda_output = Command::new("aws")
        .args([
            "lambda",
            "get-function",
            "--function-name",
            &lambda_name,
            "--region",
            &region,
            "--query",
            "Configuration.{Runtime:Runtime,Memory:MemorySize,Timeout:Timeout,Arch:Architectures[0]}",
            "--output",
            "json",
        ])
        .output();

    if let Ok(output) = lambda_output {
        if output.status.success() {
            eprintln!("\n==> Lambda Function");
            eprintln!("    Name: {}", lambda_name);

            // Parse and display Lambda details
            let json_str = String::from_utf8_lossy(&output.stdout);
            if let Ok(details) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(runtime) = details.get("Runtime").and_then(|v| v.as_str()) {
                    eprintln!("    Runtime: {}", runtime);
                }
                if let Some(memory) = details.get("Memory").and_then(|v| v.as_i64()) {
                    eprintln!("    Memory: {} MB", memory);
                }
                if let Some(timeout) = details.get("Timeout").and_then(|v| v.as_i64()) {
                    eprintln!("    Timeout: {} seconds", timeout);
                }
                if let Some(arch) = details.get("Arch").and_then(|v| v.as_str()) {
                    eprintln!("    Architecture: {}", arch);
                }
            }
        }
    }

    // Query Function URL
    let url_output = Command::new("aws")
        .args([
            "lambda",
            "get-function-url-config",
            "--function-name",
            &lambda_name,
            "--region",
            &region,
            "--query",
            "FunctionUrl",
            "--output",
            "text",
        ])
        .output();

    if let Ok(output) = url_output {
        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !url.is_empty() && !url.contains("error") {
                eprintln!("\n==> OTLP Endpoints");
                eprintln!("    POST {}v1/logs", url);
                eprintln!("    POST {}v1/traces", url);
                eprintln!("    POST {}v1/metrics", url);
            }
        }
    }

    Ok(())
}
