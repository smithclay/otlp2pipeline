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
                    "Failed to execute Azure CLI to check if Stream Analytics job '{}' exists in resource group '{}'",
                    job, rg
                )
            })?;

        if result.status.success() {
            return Ok(true);
        }

        // Parse stderr to distinguish "not found" from other errors
        let stderr = String::from_utf8(result.stderr)
            .context("Azure CLI returned invalid UTF-8 in error output")?;

        // Stream Analytics job not found is the expected "doesn't exist" case
        if stderr.contains("JobNotFound")
            || stderr.contains("ResourceNotFound")
            || stderr.contains("could not be found")
            || stderr.to_lowercase().contains("not found")
        {
            return Ok(false);
        }

        // Any other error should propagate with context
        anyhow::bail!(
            "Failed to check if Stream Analytics job '{}' exists in resource group '{}': {}",
            job,
            rg,
            stderr.trim()
        );
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!("Failed to get state for job '{}': {}", job, stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Azure CLI returned invalid UTF-8 in output")?;
        Ok(stdout.trim().to_string())
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!("Failed to delete job '{}': {}", job, stderr.trim());
        }

        Ok(())
    }

    /// Create Event Hub input
    pub fn create_input(&self, job: &str, rg: &str, config: &EventHubInputConfig) -> Result<()> {
        let eventhub_key = extract_key(config.connection_string(), "SharedAccessKey=")?;

        let input_json = json!({
            "type": "Stream",
            "datasource": {
                "type": "Microsoft.ServiceBus/EventHub",
                "properties": {
                    "serviceBusNamespace": config.namespace(),
                    "eventHubName": config.name(),
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
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!("Failed to create input: {}", stderr);
        }

        Ok(())
    }

    /// Create Parquet output for a specific container
    /// Uses Azure REST API directly to avoid CLI extension bugs with type conversion
    pub fn create_output(&self, job: &str, rg: &str, config: &ParquetOutputConfig) -> Result<()> {
        let account_key = extract_key(&config.storage_connection_string, "AccountKey=")?;

        // Get subscription ID
        let subscription_output = Command::new("az")
            .args(["account", "show", "--query", "id", "-o", "tsv"])
            .output()
            .context(
                "Failed to get subscription ID. \
                 Ensure Azure CLI is installed and you're logged in with 'az login'.",
            )?;

        if !subscription_output.status.success() {
            let stderr = String::from_utf8(subscription_output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!(
                "Failed to retrieve subscription ID: {}. \
                 Please run 'az login' to authenticate.",
                stderr.trim()
            );
        }

        let subscription_id = String::from_utf8(subscription_output.stdout)
            .context("Azure CLI returned invalid UTF-8 for subscription ID")?
            .trim()
            .to_string();

        if subscription_id.is_empty() {
            anyhow::bail!(
                "No Azure subscription found. \
                 Please run 'az login' and select a subscription with 'az account set -s <subscription-id>'."
            );
        }

        // Optional: Validate UUID format
        if !subscription_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            anyhow::bail!(
                "Invalid subscription ID format: '{}'. Expected a UUID format.",
                subscription_id
            );
        }

        // Create output properties per Azure REST API spec
        let output_body = json!({
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

        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let body_path = temp_dir.join(format!(
            "stream-analytics-output-{}.json",
            &config.output_name
        ));
        std::fs::write(&body_path, serde_json::to_string(&output_body)?)?;

        let body_arg = format!("@{}", body_path.display());
        let url = format!(
            "/subscriptions/{}/resourceGroups/{}/providers/Microsoft.StreamAnalytics/streamingjobs/{}/outputs/{}?api-version=2020-03-01",
            subscription_id, rg, job, &config.output_name
        );

        // Use az rest to call Azure REST API directly
        let output = Command::new("az")
            .args([
                "rest", "--method", "put", "--url", &url, "--body", &body_arg,
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to create Stream Analytics output '{}' for job '{}'",
                    config.output_name, job
                )
            })?;

        // Clean up temp file containing credentials - CRITICAL for security
        std::fs::remove_file(&body_path).with_context(|| {
            format!(
                "Failed to clean up temporary file containing Azure credentials at {}. \
                 For security, this file must be removed before continuing. \
                 Please manually delete this file and try again.",
                body_path.display()
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
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
                "--saql",
                query,
                "--streaming-units",
                "1",
            ])
            .output()
            .with_context(|| {
                format!(
                    "Failed to set query for Stream Analytics job '{}' in resource group '{}'",
                    job, rg
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)
                .context("Azure CLI returned invalid UTF-8 in error output")?;
            anyhow::bail!("Failed to set query: {}", stderr.trim());
        }

        Ok(())
    }
}

/// Event Hub input configuration
pub struct EventHubInputConfig {
    eventhub_namespace: String,
    eventhub_name: String,
    eventhub_connection_string: String,
}

impl EventHubInputConfig {
    /// Create a new EventHubInputConfig with validation
    pub fn new(
        eventhub_namespace: String,
        eventhub_name: String,
        eventhub_connection_string: String,
    ) -> Result<Self> {
        // Validate connection string contains SharedAccessKey
        extract_key(&eventhub_connection_string, "SharedAccessKey=")
            .context("Invalid Event Hub connection string: missing SharedAccessKey")?;
        // Validate namespace and hub name are non-empty
        if eventhub_namespace.trim().is_empty() {
            anyhow::bail!("Event Hub namespace cannot be empty");
        }
        if eventhub_name.trim().is_empty() {
            anyhow::bail!("Event Hub name cannot be empty");
        }
        Ok(Self {
            eventhub_namespace,
            eventhub_name,
            eventhub_connection_string,
        })
    }

    pub fn namespace(&self) -> &str {
        &self.eventhub_namespace
    }

    pub fn name(&self) -> &str {
        &self.eventhub_name
    }

    pub fn connection_string(&self) -> &str {
        &self.eventhub_connection_string
    }
}

/// Parquet output configuration
pub struct ParquetOutputConfig {
    pub output_name: String,
    pub storage_account: String,
    pub container: String,
    pub storage_connection_string: String,
}

/// Extract key from connection string by prefix (e.g., "SharedAccessKey=", "AccountKey=")
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
