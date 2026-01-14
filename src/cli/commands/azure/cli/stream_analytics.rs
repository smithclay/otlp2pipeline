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
            .context("Failed to check Stream Analytics job")?;

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
            .context("Failed to get Stream Analytics job state")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to get job state: {}", stderr);
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
                "--events-outoforder-policy",
                "Adjust",
                "--events-outoforder-max-delay",
                "10",
                "--events-late-arrival-max-delay",
                "5",
            ])
            .output()
            .context("Failed to create Stream Analytics job")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create job: {}", stderr);
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
            .context("Failed to start Stream Analytics job")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to start job: {}", stderr);
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
            .context("Failed to stop Stream Analytics job")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to stop job: {}", stderr);
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
            .context("Failed to delete Stream Analytics job")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to delete job: {}", stderr);
        }

        Ok(())
    }

    /// Create Event Hub input
    pub fn create_input(&self, job: &str, rg: &str, config: &EventHubInputConfig) -> Result<()> {
        let eventhub_key = extract_key(&config.eventhub_connection_string, "SharedAccessKey=")?;

        let input_json = json!({
            "name": "eventhubinput",
            "properties": {
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

        let output_json = json!({
            "name": config.output_name,
            "properties": {
                "datasource": {
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
                },
                "serialization": {
                    "type": "Parquet",
                    "properties": {}
                },
                "timeWindow": "00:05:00",
                "sizeWindow": 2000
            }
        });

        let output_str = serde_json::to_string(&output_json)?;

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
                &output_str,
            ])
            .output()
            .context("Failed to create Stream Analytics output")?;

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
