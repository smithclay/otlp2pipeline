// src/cli/commands/azure/context.rs
use anyhow::Result;

use super::cli::AzureCli;
use super::helpers::{
    container_app_name, eventhub_namespace, resource_group_name, storage_account_name,
    stream_analytics_job_name, validate_name_lengths, CONTAINERS, EVENTHUB_NAME,
};

/// Deployment context containing all resource names and IDs
pub struct DeployContext {
    pub subscription_id: String,
    pub env_name: String,
    pub region: String,
    pub resource_group: String,
    pub storage_account: String,
    pub eventhub_namespace: String,
    pub eventhub_name: String,
    pub stream_analytics_job: String,
    pub containers: Vec<String>,
    pub container_app_name: String,
    pub container_image: String,
    pub auth_token: Option<String>,
}

impl DeployContext {
    /// Create new deployment context with generated resource names
    pub fn new(
        cli: &AzureCli,
        env_name: &str,
        region: &str,
        resource_group: Option<String>,
    ) -> Result<Self> {
        // Validate names before proceeding
        validate_name_lengths(env_name, region)?;

        let subscription_id = cli.account().get_subscription_id()?;
        let rg = resource_group.unwrap_or_else(|| resource_group_name(env_name));

        Ok(Self {
            subscription_id,
            env_name: env_name.to_string(),
            region: region.to_string(),
            resource_group: rg,
            storage_account: storage_account_name(env_name)?,
            eventhub_namespace: eventhub_namespace(env_name),
            eventhub_name: EVENTHUB_NAME.to_string(),
            stream_analytics_job: stream_analytics_job_name(env_name),
            containers: CONTAINERS.iter().map(|s| s.to_string()).collect(),
            container_app_name: container_app_name(env_name),
            container_image: "ghcr.io/smithclay/otlp2pipeline:v1-amd64".to_string(),
            auth_token: None,
        })
    }
}
