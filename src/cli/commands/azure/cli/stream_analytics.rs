// src/cli/commands/azure/cli/stream_analytics.rs
use anyhow::{Context, Result};
use serde_json::json;
use std::process::Command;

pub struct StreamAnalyticsCli {
    region: String,
}

impl StreamAnalyticsCli {
    pub fn new(region: &str) -> Self {
        Self {
            region: region.to_string(),
        }
    }

    /// Check if Stream Analytics job exists
    pub fn job_exists(&self, job: &str, rg: &str) -> Result<bool> {
        let result = Command::new("az")
            .args([
                "stream-analytics",
                "job",
                "show",
                "--name",
                job,
                "--resource-group",
                rg,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to check if Stream Analytics job '{}' exists in resource group '{}'",
                    job, rg
                )
            })?;

        Ok(result.status.success())
    }

    /// Get Stream Analytics job state
    pub fn get_job_state(&self, job: &str, rg: &str) -> Result<String> {
        let output = Command::new("az")
            .args([
                "stream-analytics",
                "job",
                "show",
                "--name",
                job,
                "--resource-group",
                rg,
                "--query",
                "jobState",
                "-o",
                "tsv",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to get state for Stream Analytics job '{}' in resource group '{}'",
                    job, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to get state for job '{}': {}", job, stderr.trim());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Create Stream Analytics job
    pub fn create_job(&self, job: &str, rg: &str) -> Result<()> {
        let output = Command::new("az")
            .args([
                "stream-analytics",
                "job",
                "create",
                "--name",
                job,
                "--resource-group",
                rg,
                "--location",
                &self.region,
                "--output-error-policy",
                "Drop",
                "--out-of-order-policy",
                "Adjust",
                "--order-max-delay",
                "10",
                "--arrival-max-delay",
                "5",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to create Stream Analytics job '{}' in resource group '{}'",
                    job, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create job '{}': {}", job, stderr.trim());
        }

        Ok(())
    }

    /// Start Stream Analytics job
    pub fn start_job(&self, job: &str, rg: &str) -> Result<()> {
        let output = Command::new("az")
            .args([
                "stream-analytics",
                "job",
                "start",
                "--name",
                job,
                "--resource-group",
                rg,
                "--output-start-mode",
                "JobStartTime",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to start Stream Analytics job '{}' in resource group '{}'",
                    job, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to start job '{}': {}", job, stderr.trim());
        }

        Ok(())
    }

    /// Stop Stream Analytics job
    pub fn stop_job(&self, job: &str, rg: &str) -> Result<()> {
        let output = Command::new("az")
            .args([
                "stream-analytics",
                "job",
                "stop",
                "--name",
                job,
                "--resource-group",
                rg,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to stop Stream Analytics job '{}' in resource group '{}'",
                    job, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to stop job '{}': {}", job, stderr.trim());
        }

        Ok(())
    }

    /// Delete Stream Analytics job
    pub fn delete_job(&self, job: &str, rg: &str) -> Result<()> {
        let output = Command::new("az")
            .args([
                "stream-analytics",
                "job",
                "delete",
                "--name",
                job,
                "--resource-group",
                rg,
                "--yes",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to delete Stream Analytics job '{}' in resource group '{}'",
                    job, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to delete job '{}': {}", job, stderr.trim());
        }

        Ok(())
    }

    /// Create Event Hub input
    pub fn create_input(&self, job: &str, rg: &str, config: &EventHubInputConfig) -> Result<()> {
        let eventhub_key = extract_key(&config.eventhub_connection_string, "SharedAccessKey=")?;

        let input_json = json!({
            "type": "Stream",
            "datasource": {
                "type": "Microsoft.ServiceBus/EventHub",
                "properties": {
                    "serviceBusNamespace": config.eventhub_namespace,
                    "eventHubName": config.eventhub_name,
                    "sharedAccessPolicyName": "RootManageSharedAccessKey",
                    "sharedAccessPolicyKey": eventhub_key,
                    "consumerGroupName": "$Default"
                }
            },
            "serialization": {
                "type": "Json",
                "properties": {
                    "encoding": "UTF8"
                }
            }
        });

        let input_str = serde_json::to_string(&input_json)?;

        let output = Command::new("az")
            .args([
                "stream-analytics",
                "input",
                "create",
                "--job-name",
                job,
                "--resource-group",
                rg,
                "--name",
                "eventhubinput",
                "--properties",
                &input_str,
            ])
            .output()
            .context("Failed to create Stream Analytics input")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create input: {}", stderr);
        }

        Ok(())
    }

    /// Create Parquet output for a specific container
    pub fn create_output(&self, job: &str, rg: &str, config: &ParquetOutputConfig) -> Result<()> {
        let account_key = extract_account_key(&config.storage_connection_string)?;

        let datasource_json = json!({
            "type": "Microsoft.Storage/Blob",
            "properties": {
                "storageAccounts": [{
                    "accountName": config.storage_account,
                    "accountKey": account_key
                }],
                "container": config.container,
                "pathPattern": "{date}/{time}",
                "dateFormat": "yyyy/MM/dd",
                "timeFormat": "HH"
            }
        });

        let serialization_json = json!({
            "type": "Parquet",
            "properties": {
                "timeWindow": "00:05:00",
                "sizeWindow": 2000
            }
        });

        let datasource_str = serde_json::to_string(&datasource_json)?;
        let serialization_str = serde_json::to_string(&serialization_json)?;

        let output = Command::new("az")
            .args([
                "stream-analytics",
                "output",
                "create",
                "--job-name",
                job,
                "--resource-group",
                rg,
                "--name",
                &config.output_name,
                "--datasource",
                &datasource_str,
                "--serialization",
                &serialization_str,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to create Stream Analytics output '{}' for job '{}'",
                    config.output_name, job
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to create output '{}': {}",
                config.output_name,
                stderr
            );
        }

        Ok(())
    }

    /// Set Stream Analytics transformation query
    pub fn set_query(&self, job: &str, rg: &str, query: &str) -> Result<()> {
        let transformation_json = json!({
            "name": "Transformation",
            "properties": {
                "streamingUnits": 1,
                "query": query
            }
        });

        let transformation_str = serde_json::to_string(&transformation_json)?;

        let output = Command::new("az")
            .args([
                "stream-analytics",
                "transformation",
                "create",
                "--job-name",
                job,
                "--resource-group",
                rg,
                "--name",
                "Transformation",
                "--transformation",
                &transformation_str,
            ])
            .output()
            .context("Failed to set Stream Analytics query")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to set query: {}", stderr);
        }

        Ok(())
    }
}

/// Event Hub input configuration
pub struct EventHubInputConfig {
    pub eventhub_namespace: String,
    pub eventhub_name: String,
    pub eventhub_connection_string: String,
}

/// Parquet output configuration
pub struct ParquetOutputConfig {
    pub output_name: String,
    pub storage_account: String,
    pub container: String,
    pub storage_connection_string: String,
}

/// Extract SharedAccessKey from Event Hub connection string
fn extract_key(connection_string: &str, prefix: &str) -> Result<String> {
    connection_string
        .split(';')
        .find(|part| part.starts_with(prefix))
        .and_then(|part| part.strip_prefix(prefix))
        .map(|s| s.to_string())
        .context(format!(
            "Failed to extract {} from connection string",
            prefix
        ))
}

/// Extract account key from storage connection string
fn extract_account_key(connection_string: &str) -> Result<String> {
    extract_key(connection_string, "AccountKey=")
}
