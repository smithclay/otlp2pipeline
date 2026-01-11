use super::{run_idempotent, run_optional, AwsCli};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

pub struct LambdaCli<'a> {
    pub(super) aws: &'a AwsCli,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FunctionUrlResponse {
    function_url: String,
}

pub struct LambdaConfig {
    pub name: String,
    pub role_arn: String,
    pub s3_bucket: String,
    pub s3_key: String,
    pub memory_size: u32,
    pub timeout: u32,
    pub environment: Vec<(String, String)>,
}

impl LambdaCli<'_> {
    pub fn function_exists(&self, name: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "get-function",
            "--function-name",
            name,
            "--region",
            self.aws.region(),
        ]);
        let result = run_optional(&mut cmd, &["ResourceNotFoundException"])?;
        Ok(result.is_some())
    }

    pub fn get_function_url(&self, name: &str) -> Result<Option<String>> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "get-function-url-config",
            "--function-name",
            name,
            "--region",
            self.aws.region(),
            "--output",
            "json",
        ]);
        let result = run_optional(&mut cmd, &["ResourceNotFoundException"])?;
        match result {
            Some(json) => {
                let response: FunctionUrlResponse = serde_json::from_str(&json)?;
                Ok(Some(response.function_url))
            }
            None => Ok(None),
        }
    }

    pub fn create_function(&self, config: &LambdaConfig) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "create-function",
            "--function-name",
            &config.name,
            "--runtime",
            "provided.al2023",
            "--architectures",
            "arm64",
            "--handler",
            "bootstrap",
            "--role",
            &config.role_arn,
            "--memory-size",
            &config.memory_size.to_string(),
            "--timeout",
            &config.timeout.to_string(),
            "--code",
            &format!("S3Bucket={},S3Key={}", config.s3_bucket, config.s3_key),
            "--region",
            self.aws.region(),
        ]);
        if !config.environment.is_empty() {
            // Use JSON format for env vars to handle special characters safely
            let vars: HashMap<&str, &str> = config
                .environment
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let env_json = serde_json::json!({ "Variables": vars }).to_string();
            cmd.args(["--environment", &env_json]);
        }
        run_idempotent(&mut cmd, &["ResourceConflictException"])?;
        Ok(())
    }

    pub fn update_function_code(&self, name: &str, s3_bucket: &str, s3_key: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "update-function-code",
            "--function-name",
            name,
            "--s3-bucket",
            s3_bucket,
            "--s3-key",
            s3_key,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }

    /// Wait for Lambda function to be ready after code/config update
    pub fn wait_function_updated(&self, name: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "wait",
            "function-updated-v2",
            "--function-name",
            name,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }

    pub fn create_function_url(&self, name: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "create-function-url-config",
            "--function-name",
            name,
            "--auth-type",
            "NONE",
            "--region",
            self.aws.region(),
        ]);
        let output = run_idempotent(&mut cmd, &["ResourceConflictException"])?;
        Ok(!output.already_existed)
    }

    pub fn add_public_url_permission(&self, name: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "add-permission",
            "--function-name",
            name,
            "--statement-id",
            "FunctionURLAllowPublicAccess",
            "--action",
            "lambda:InvokeFunctionUrl",
            "--principal",
            "*",
            "--function-url-auth-type",
            "NONE",
            "--region",
            self.aws.region(),
        ]);
        let output = run_idempotent(&mut cmd, &["ResourceConflictException"])?;
        Ok(!output.already_existed)
    }

    /// Update Lambda function environment variables
    pub fn update_function_configuration(
        &self,
        name: &str,
        env_vars: &[(String, String)],
    ) -> Result<()> {
        // Use JSON format for env vars to handle special characters safely
        let vars: HashMap<&str, &str> = env_vars
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let env_json = serde_json::json!({ "Variables": vars }).to_string();

        let mut cmd = Command::new("aws");
        cmd.args([
            "lambda",
            "update-function-configuration",
            "--function-name",
            name,
            "--environment",
            &env_json,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }
}
