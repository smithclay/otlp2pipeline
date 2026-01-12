use super::{run_json, AwsCli};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub struct AthenaCli<'a> {
    pub(super) aws: &'a AwsCli,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StartQueryResponse {
    query_execution_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct GetQueryExecutionResponse {
    query_execution: QueryExecution,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QueryExecution {
    status: QueryStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QueryStatus {
    state: String,
    state_change_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum QueryState {
    Succeeded,
    Failed(String),
    Cancelled,
    Running,
    Queued,
}

impl AthenaCli<'_> {
    pub fn start_query_execution(
        &self,
        query: &str,
        catalog: &str,
        output_location: &str,
    ) -> Result<String> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "athena",
            "start-query-execution",
            "--query-string",
            query,
            "--query-execution-context",
            &format!("Catalog={}", catalog),
            "--result-configuration",
            &format!("OutputLocation={}", output_location),
            "--region",
            self.aws.region(),
            "--output",
            "json",
        ]);
        let response: StartQueryResponse = run_json(&mut cmd)?;
        Ok(response.query_execution_id)
    }

    pub fn get_query_state(&self, query_id: &str) -> Result<QueryState> {
        let mut cmd = Command::new("aws");
        cmd.args([
            "athena",
            "get-query-execution",
            "--query-execution-id",
            query_id,
            "--region",
            self.aws.region(),
            "--output",
            "json",
        ]);
        let response: GetQueryExecutionResponse = run_json(&mut cmd)?;
        let status = response.query_execution.status;
        match status.state.as_str() {
            "SUCCEEDED" => Ok(QueryState::Succeeded),
            "FAILED" => Ok(QueryState::Failed(
                status
                    .state_change_reason
                    .unwrap_or_else(|| "unknown".to_string()),
            )),
            "CANCELLED" => Ok(QueryState::Cancelled),
            "RUNNING" => Ok(QueryState::Running),
            "QUEUED" => Ok(QueryState::Queued),
            other => bail!("Unknown query state: {}", other),
        }
    }

    pub fn wait_query_complete(&self, query_id: &str, timeout_secs: u64) -> Result<QueryState> {
        let max_attempts = timeout_secs / 2;
        for _ in 0..max_attempts {
            let state = self.get_query_state(query_id)?;
            match &state {
                QueryState::Succeeded | QueryState::Failed(_) | QueryState::Cancelled => {
                    return Ok(state)
                }
                QueryState::Running | QueryState::Queued => thread::sleep(Duration::from_secs(2)),
            }
        }
        bail!("Query timed out after {} seconds", timeout_secs);
    }

    pub fn execute_query(
        &self,
        query: &str,
        catalog: &str,
        output_location: &str,
    ) -> Result<QueryState> {
        let query_id = self.start_query_execution(query, catalog, output_location)?;
        self.wait_query_complete(&query_id, 120)
    }
}
