use super::{run_idempotent, run_optional, AwsCli};
use anyhow::Result;
use std::process::Command;

pub struct LakeFormationCli<'a> {
    pub(super) aws: &'a AwsCli,
}

impl LakeFormationCli<'_> {
    pub fn put_data_lake_settings(&self, admin_arns: &[&str]) -> Result<()> {
        let admins: Vec<serde_json::Value> = admin_arns
            .iter()
            .map(|arn| serde_json::json!({"DataLakePrincipalIdentifier": arn}))
            .collect();
        let settings = serde_json::json!({"DataLakeAdmins": admins});
        let mut cmd = Command::new("aws");
        cmd.args([
            "lakeformation",
            "put-data-lake-settings",
            "--data-lake-settings",
            &settings.to_string(),
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }

    pub fn deregister_resource(&self, resource_arn: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lakeformation",
            "deregister-resource",
            "--resource-arn",
            resource_arn,
            "--region",
            self.aws.region(),
        ]);
        // EntityNotFoundException is the specific AWS exception for unregistered resources
        run_idempotent(&mut cmd, &["EntityNotFoundException"])?;
        Ok(())
    }

    pub fn register_resource(&self, resource_arn: &str, role_arn: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lakeformation",
            "register-resource",
            "--resource-arn",
            resource_arn,
            "--role-arn",
            role_arn,
            "--with-federation",
            "--region",
            self.aws.region(),
        ]);
        let output = run_idempotent(&mut cmd, &["AlreadyExistsException"])?;
        Ok(!output.already_existed)
    }

    pub fn describe_resource(&self, resource_arn: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "lakeformation",
            "describe-resource",
            "--resource-arn",
            resource_arn,
            "--region",
            self.aws.region(),
        ]);
        let result = run_optional(&mut cmd, &["EntityNotFoundException"])?;
        Ok(result.is_some())
    }

    pub fn grant_permissions(
        &self,
        principal_arn: &str,
        resource: &serde_json::Value,
        permissions: &[&str],
        with_grant: bool,
    ) -> Result<bool> {
        let principal = serde_json::json!({"DataLakePrincipalIdentifier": principal_arn});
        let mut cmd = Command::new("aws");
        cmd.args([
            "lakeformation",
            "grant-permissions",
            "--principal",
            &principal.to_string(),
            "--resource",
            &resource.to_string(),
            "--permissions",
        ]);
        cmd.args(permissions);
        if with_grant {
            cmd.arg("--permissions-with-grant-option");
            cmd.args(permissions);
        }
        cmd.args(["--region", self.aws.region()]);
        let output = run_idempotent(&mut cmd, &["AlreadyExistsException"])?;
        Ok(!output.already_existed)
    }
}
