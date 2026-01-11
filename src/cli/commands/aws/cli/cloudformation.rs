use super::{run_idempotent, run_optional, AwsCli};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

pub struct CloudFormationCli<'a> {
    pub(super) aws: &'a AwsCli,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DescribeStacksResponse {
    stacks: Vec<StackDescription>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StackDescription {
    stack_status: String,
    outputs: Option<Vec<StackOutput>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StackOutput {
    output_key: String,
    output_value: String,
}

#[derive(Debug, Clone)]
pub struct StackInfo {
    pub status: String,
    pub outputs: HashMap<String, String>,
}

impl CloudFormationCli<'_> {
    pub fn deploy(
        &self,
        stack_name: &str,
        template_body: &str,
        params: &[(&str, &str)],
    ) -> Result<()> {
        let temp_path = std::env::temp_dir().join(format!("cfn-{}.yaml", stack_name));
        std::fs::write(&temp_path, template_body)?;

        let temp_path_str = temp_path.to_string_lossy();
        let mut cmd = Command::new("aws");
        cmd.args([
            "cloudformation",
            "deploy",
            "--template-file",
            &temp_path_str,
            "--stack-name",
            stack_name,
            "--region",
            self.aws.region(),
            "--capabilities",
            "CAPABILITY_NAMED_IAM",
            "--no-fail-on-empty-changeset",
        ]);
        if !params.is_empty() {
            cmd.arg("--parameter-overrides");
            for (key, value) in params {
                cmd.arg(format!("{}={}", key, value));
            }
        }

        let output = cmd.output()?;
        if let Err(e) = std::fs::remove_file(&temp_path) {
            eprintln!(
                "    Warning: Could not remove temp file {}: {}",
                temp_path.display(),
                e
            );
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("CloudFormation deploy failed: {}", stderr.trim());
        }
        Ok(())
    }

    pub fn describe_stack(&self, stack_name: &str) -> Result<Option<StackInfo>> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "cloudformation",
            "describe-stacks",
            "--stack-name",
            stack_name,
            "--region",
            self.aws.region(),
            "--output",
            "json",
        ]);
        let result = run_optional(&mut cmd, &["does not exist"])?;
        match result {
            Some(json) => {
                let response: DescribeStacksResponse = serde_json::from_str(&json)?;
                if let Some(stack) = response.stacks.first() {
                    let outputs = stack
                        .outputs
                        .as_ref()
                        .map(|o| {
                            o.iter()
                                .map(|out| (out.output_key.clone(), out.output_value.clone()))
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(Some(StackInfo {
                        status: stack.stack_status.clone(),
                        outputs,
                    }))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    pub fn get_stack_output(&self, stack_name: &str, output_key: &str) -> Result<Option<String>> {
        let info = self.describe_stack(stack_name)?;
        Ok(info.and_then(|i| i.outputs.get(output_key).cloned()))
    }

    pub fn delete_stack(&self, stack_name: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "cloudformation",
            "delete-stack",
            "--stack-name",
            stack_name,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &["does not exist"])?;
        Ok(())
    }

    pub fn wait_stack_create_complete(&self, stack_name: &str) -> Result<()> {
        let output = Command::new("aws")
            .args([
                "cloudformation",
                "wait",
                "stack-create-complete",
                "--stack-name",
                stack_name,
                "--region",
                self.aws.region(),
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Stack creation did not complete successfully.\n\
                 {}\n\n\
                 Run `aws cloudformation describe-stack-events --stack-name {}` for details",
                stderr.trim(),
                stack_name
            );
        }
        Ok(())
    }

    pub fn wait_stack_delete_complete(&self, stack_name: &str) -> Result<()> {
        let output = Command::new("aws")
            .args([
                "cloudformation",
                "wait",
                "stack-delete-complete",
                "--stack-name",
                stack_name,
                "--region",
                self.aws.region(),
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Stack deletion did not complete successfully.\n\
                 {}\n\n\
                 Run `aws cloudformation describe-stack-events --stack-name {}` for details",
                stderr.trim(),
                stack_name
            );
        }
        Ok(())
    }
}
