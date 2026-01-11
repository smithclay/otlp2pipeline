use super::cli::AwsCli;
use super::helpers::stack_name;
use anyhow::Result;
use std::collections::HashMap;

/// S3 Tables IAM role name (global, shared across stacks)
pub const S3_TABLES_ROLE_NAME: &str = "S3TablesRoleForLakeFormation";

/// Trust policy for LakeFormation
pub fn s3_tables_trust_policy() -> serde_json::Value {
    serde_json::json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {"Service": "lakeformation.amazonaws.com"},
            "Action": ["sts:AssumeRole", "sts:SetSourceIdentity", "sts:SetContext"]
        }]
    })
}

/// S3 Tables data access policy
pub fn s3_tables_data_policy() -> serde_json::Value {
    serde_json::json!({
        "Version": "2012-10-17",
        "Statement": [
            {
                "Sid": "LakeFormationPermissionsForS3ListTableBucket",
                "Effect": "Allow",
                "Action": ["s3tables:ListTableBuckets"],
                "Resource": ["*"]
            },
            {
                "Sid": "LakeFormationDataAccessPermissionsForS3TableBucket",
                "Effect": "Allow",
                "Action": [
                    "s3tables:CreateTableBucket",
                    "s3tables:GetTableBucket",
                    "s3tables:CreateNamespace",
                    "s3tables:GetNamespace",
                    "s3tables:ListNamespaces",
                    "s3tables:DeleteNamespace",
                    "s3tables:DeleteTableBucket",
                    "s3tables:CreateTable",
                    "s3tables:DeleteTable",
                    "s3tables:GetTable",
                    "s3tables:ListTables",
                    "s3tables:RenameTable",
                    "s3tables:UpdateTableMetadataLocation",
                    "s3tables:GetTableMetadataLocation",
                    "s3tables:GetTableData",
                    "s3tables:PutTableData"
                ],
                "Resource": ["*"]
            }
        ]
    })
}

/// Deployment context passed through all phases
#[derive(Debug, Clone)]
pub struct DeployContext {
    pub account_id: String,
    pub caller_arn: String,
    pub region: String,
    pub env_name: String,
    pub stack_name: String,
    pub bucket_name: String,
    pub namespace: String,
    pub local_build: bool,
    pub auth_token: Option<String>,
    stack_outputs: HashMap<String, String>,
}

impl DeployContext {
    /// Create new context by fetching account info
    pub fn new(cli: &AwsCli, env_name: &str, namespace: &str, local_build: bool) -> Result<Self> {
        let account = cli.sts().get_caller_identity()?;
        let stack = stack_name(env_name);
        let bucket = stack.clone(); // bucket name = stack name

        Ok(Self {
            account_id: account.account_id,
            caller_arn: account.caller_arn,
            region: cli.region().to_string(),
            env_name: env_name.to_string(),
            stack_name: stack,
            bucket_name: bucket,
            namespace: namespace.to_string(),
            local_build,
            auth_token: None,
            stack_outputs: HashMap::new(),
        })
    }

    /// S3 Tables resource ARN (wildcard for all buckets)
    pub fn s3_tables_resource_arn(&self) -> String {
        format!(
            "arn:aws:s3tables:{}:{}:bucket/*",
            self.region, self.account_id
        )
    }

    /// S3 Tables role ARN
    pub fn s3_tables_role_arn(&self) -> String {
        format!(
            "arn:aws:iam::{}:role/{}",
            self.account_id, S3_TABLES_ROLE_NAME
        )
    }

    /// Glue catalog ARN for S3 Tables
    pub fn glue_catalog_arn(&self) -> String {
        format!(
            "arn:aws:glue:{}:{}:catalog/s3tablescatalog/{}",
            self.region, self.account_id, self.bucket_name
        )
    }

    /// S3 Tables bucket ARN
    pub fn table_bucket_arn(&self) -> String {
        format!(
            "arn:aws:s3tables:{}:{}:bucket/{}",
            self.region, self.account_id, self.bucket_name
        )
    }

    /// Error bucket name
    pub fn error_bucket_name(&self) -> String {
        format!(
            "{}-errors-{}-{}",
            self.stack_name, self.account_id, self.region
        )
    }

    /// Artifact bucket name
    pub fn artifact_bucket_name(&self) -> String {
        format!("{}-artifacts-{}", self.stack_name, self.account_id)
    }

    /// Lambda function name
    pub fn lambda_function_name(&self) -> String {
        format!("{}-ingest", self.stack_name)
    }

    /// Lambda role ARN
    pub fn lambda_role_arn(&self) -> String {
        format!(
            "arn:aws:iam::{}:role/{}-Lambda-{}",
            self.account_id, self.stack_name, self.region
        )
    }

    /// Firehose stream name for a signal
    pub fn firehose_stream_name(&self, signal: &str) -> String {
        format!("{}-{}", self.stack_name, signal)
    }

    /// Set stack outputs after CFN deploy
    pub fn set_stack_outputs(&mut self, outputs: HashMap<String, String>) {
        self.stack_outputs = outputs;
    }

    /// Get a stack output value
    pub fn get_output(&self, key: &str) -> Option<&String> {
        self.stack_outputs.get(key)
    }
}
