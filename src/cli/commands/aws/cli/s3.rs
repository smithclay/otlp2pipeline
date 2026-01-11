use super::{run_idempotent, AwsCli};
use anyhow::Result;
use std::process::Command;

pub struct S3Cli<'a> {
    pub(super) aws: &'a AwsCli,
}

impl S3Cli<'_> {
    pub fn rm_recursive(&self, bucket: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "s3",
            "rm",
            &format!("s3://{}", bucket),
            "--recursive",
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &["NoSuchBucket", "does not exist"])?;
        Ok(())
    }

    pub fn cp(&self, local_path: &str, s3_uri: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "s3",
            "cp",
            local_path,
            s3_uri,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &[])?;
        Ok(())
    }
}
