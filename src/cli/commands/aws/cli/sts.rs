use super::{run_json, AwsCli};
use anyhow::Result;
use serde::Deserialize;
use std::process::Command;

pub struct StsCli<'a> {
    pub(super) aws: &'a AwsCli,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CallerIdentityResponse {
    account: String,
    arn: String,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub account_id: String,
    pub caller_arn: String,
}

impl StsCli<'_> {
    pub fn get_caller_identity(&self) -> Result<AccountInfo> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "sts",
            "get-caller-identity",
            "--region",
            self.aws.region(),
            "--output",
            "json",
        ]);
        let response: CallerIdentityResponse = run_json(&mut cmd)?;
        Ok(AccountInfo {
            account_id: response.account,
            caller_arn: response.arn,
        })
    }
}
