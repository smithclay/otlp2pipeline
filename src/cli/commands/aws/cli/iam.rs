use super::{run_idempotent, run_optional, AwsCli};
use anyhow::Result;
use std::process::Command;

pub struct IamCli<'a> {
    pub(super) aws: &'a AwsCli,
}

impl IamCli<'_> {
    pub fn create_role(&self, name: &str, trust_policy: &serde_json::Value) -> Result<bool> {
        let policy_json = serde_json::to_string(trust_policy)?;
        let mut cmd = Command::new("aws");
        cmd.args([
            "iam",
            "create-role",
            "--role-name",
            name,
            "--assume-role-policy-document",
            &policy_json,
            "--region",
            self.aws.region(),
        ]);
        let output = run_idempotent(&mut cmd, &["EntityAlreadyExists"])?;
        Ok(!output.already_existed)
    }

    pub fn update_assume_role_policy(
        &self,
        name: &str,
        trust_policy: &serde_json::Value,
    ) -> Result<()> {
        let policy_json = serde_json::to_string(trust_policy)?;
        let mut cmd = Command::new("aws");
        cmd.args([
            "iam",
            "update-assume-role-policy",
            "--role-name",
            name,
            "--policy-document",
            &policy_json,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }

    pub fn put_role_policy(
        &self,
        role: &str,
        policy_name: &str,
        policy: &serde_json::Value,
    ) -> Result<()> {
        let policy_json = serde_json::to_string(policy)?;
        let mut cmd = Command::new("aws");
        cmd.args([
            "iam",
            "put-role-policy",
            "--role-name",
            role,
            "--policy-name",
            policy_name,
            "--policy-document",
            &policy_json,
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }

    pub fn role_exists(&self, name: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "iam",
            "get-role",
            "--role-name",
            name,
            "--region",
            self.aws.region(),
        ]);
        let result = run_optional(&mut cmd, &["NoSuchEntity"])?;
        Ok(result.is_some())
    }
}
