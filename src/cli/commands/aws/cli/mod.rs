#![allow(dead_code)]

mod athena;
mod cloudformation;
mod firehose;
mod glue;
mod iam;
mod lakeformation;
mod lambda;
mod s3;
mod s3tables;
mod sts;

pub use athena::{AthenaCli, QueryState};
pub use cloudformation::CloudFormationCli;
pub use firehose::{FirehoseCli, FirehoseStreamConfig};
pub use glue::GlueCli;
pub use iam::IamCli;
pub use lakeformation::LakeFormationCli;
pub use lambda::{LambdaCli, LambdaConfig};
pub use s3::S3Cli;
pub use s3tables::S3TablesCli;
pub use sts::StsCli;

use anyhow::{bail, Result};
use serde::de::DeserializeOwned;
use std::process::Command;

/// Core AWS CLI wrapper with region context
pub struct AwsCli {
    region: String,
}

impl AwsCli {
    pub fn new(region: &str) -> Self {
        Self {
            region: region.to_string(),
        }
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    // Service accessors
    pub fn sts(&self) -> StsCli<'_> {
        StsCli { aws: self }
    }
    pub fn iam(&self) -> IamCli<'_> {
        IamCli { aws: self }
    }
    pub fn lakeformation(&self) -> LakeFormationCli<'_> {
        LakeFormationCli { aws: self }
    }
    pub fn glue(&self) -> GlueCli<'_> {
        GlueCli { aws: self }
    }
    pub fn cloudformation(&self) -> CloudFormationCli<'_> {
        CloudFormationCli { aws: self }
    }
    pub fn athena(&self) -> AthenaCli<'_> {
        AthenaCli { aws: self }
    }
    pub fn firehose(&self) -> FirehoseCli<'_> {
        FirehoseCli { aws: self }
    }
    pub fn s3tables(&self) -> S3TablesCli<'_> {
        S3TablesCli { aws: self }
    }
    pub fn s3(&self) -> S3Cli<'_> {
        S3Cli { aws: self }
    }
    pub fn lambda(&self) -> LambdaCli<'_> {
        LambdaCli { aws: self }
    }
}

/// Output from an idempotent command
#[derive(Debug)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub already_existed: bool,
}

/// Run command, treating specific error patterns as "already exists"
pub fn run_idempotent(cmd: &mut Command, expected_errors: &[&str]) -> Result<CommandOutput> {
    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        return Ok(CommandOutput {
            stdout,
            stderr,
            already_existed: false,
        });
    }

    for pattern in expected_errors {
        if stderr.contains(pattern) || stdout.contains(pattern) {
            return Ok(CommandOutput {
                stdout,
                stderr,
                already_existed: true,
            });
        }
    }

    bail!("Command failed: {}", stderr.trim());
}

/// Run command and parse JSON output
pub fn run_json<T: DeserializeOwned>(cmd: &mut Command) -> Result<T> {
    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Command failed: {}", stderr.trim());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: T = serde_json::from_str(&stdout)?;
    Ok(parsed)
}

/// Run command and return stdout as string, or None if command fails with expected error
pub fn run_optional(cmd: &mut Command, not_found_errors: &[&str]) -> Result<Option<String>> {
    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        return Ok(Some(stdout));
    }

    for pattern in not_found_errors {
        if stderr.contains(pattern) || stdout.contains(pattern) {
            return Ok(None);
        }
    }

    bail!("Command failed: {}", stderr.trim());
}
