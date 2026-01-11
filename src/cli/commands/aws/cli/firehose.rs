use super::{run_idempotent, run_optional, AwsCli};
use anyhow::Result;
use std::process::Command;

pub struct FirehoseCli<'a> {
    pub(super) aws: &'a AwsCli,
}

pub struct FirehoseStreamConfig {
    pub name: String,
    pub role_arn: String,
    pub catalog_arn: String,
    pub database: String,
    pub table: String,
    pub log_group: String,
    pub log_stream: String,
    pub error_bucket: String,
    pub error_prefix: String,
    pub batch_interval_secs: u32,
    pub batch_size_mb: u32,
}

impl FirehoseCli<'_> {
    pub fn stream_exists(&self, name: &str) -> Result<bool> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "firehose",
            "describe-delivery-stream",
            "--delivery-stream-name",
            name,
            "--region",
            self.aws.region(),
        ]);
        let result = run_optional(&mut cmd, &["ResourceNotFoundException"])?;
        Ok(result.is_some())
    }

    pub fn create_delivery_stream(&self, config: &FirehoseStreamConfig) -> Result<bool> {
        if self.stream_exists(&config.name)? {
            return Ok(false);
        }

        let iceberg_config = serde_json::json!({
            "RoleARN": config.role_arn,
            "CatalogConfiguration": {"CatalogARN": config.catalog_arn},
            "DestinationTableConfigurationList": [{"DestinationDatabaseName": config.database, "DestinationTableName": config.table}],
            "BufferingHints": {"IntervalInSeconds": config.batch_interval_secs, "SizeInMBs": config.batch_size_mb},
            "CloudWatchLoggingOptions": {"Enabled": true, "LogGroupName": config.log_group, "LogStreamName": config.log_stream},
            "S3Configuration": {"RoleARN": config.role_arn, "BucketARN": format!("arn:aws:s3:::{}", config.error_bucket), "ErrorOutputPrefix": format!("{}{}/", config.error_prefix, config.table)}
        });

        let mut cmd = Command::new("aws");
        cmd.args([
            "firehose",
            "create-delivery-stream",
            "--delivery-stream-name",
            &config.name,
            "--delivery-stream-type",
            "DirectPut",
            "--iceberg-destination-configuration",
            &iceberg_config.to_string(),
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &["ResourceInUseException"])?;
        Ok(true)
    }

    pub fn delete_delivery_stream(&self, name: &str) -> Result<()> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "firehose",
            "delete-delivery-stream",
            "--delivery-stream-name",
            name,
            "--region",
            self.aws.region(),
        ]);
        run_idempotent(&mut cmd, &["ResourceNotFoundException"])?;
        Ok(())
    }
}
