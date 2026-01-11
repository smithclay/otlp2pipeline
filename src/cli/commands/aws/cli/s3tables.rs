use super::{run_idempotent, AwsCli};
use anyhow::Result;
use std::process::Command;

pub struct S3TablesCli<'a> {
    pub(super) aws: &'a AwsCli,
}

impl S3TablesCli<'_> {
    pub fn delete_table(&self, bucket_arn: &str, namespace: &str, table: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "s3tables",
            "delete-table",
            "--table-bucket-arn",
            bucket_arn,
            "--namespace",
            namespace,
            "--name",
            table,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &["NotFoundException", "does not exist"])?;
        Ok(())
    }
}
