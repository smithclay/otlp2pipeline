# Azure Support Design

**Date:** 2026-01-14
**Status:** Approved
**Scope:** Initial Azure support with CLI commands (create/status/destroy), Rust example script, and Stream Analytics routing

## Overview

Add Azure support to `otlp2pipeline` CLI, enabling deployment of OTLP ingestion infrastructure on Azure using Event Hubs, Stream Analytics, and ADLS Gen2. This implementation follows the existing AWS pattern for consistency.

**Out of Scope (Initial):**
- Azure Function deployment (HTTP → Event Hub)
- Full OTLP transformation pipeline
- Query/catalog operations

## Architecture

### Infrastructure Components

```
┌─────────────────────────────────────────────────────────────┐
│  Resource Group: otlp2pipeline-{env}                        │
│                                                              │
│  ┌────────────────────┐        ┌─────────────────────────┐ │
│  │  Event Hub         │        │  Stream Analytics Job   │ │
│  │  Namespace         │───────▶│  otlp-{env}-stream-     │ │
│  │  otlp-{env}-hub    │ JSON   │  processor              │ │
│  │                    │        │                         │ │
│  │  Hub:              │        │  Query routes by        │ │
│  │  otlp-ingestion    │        │  signal_type field      │ │
│  └────────────────────┘        └─────────────────────────┘ │
│                                            │                │
│                                            │ Parquet        │
│                                            ▼                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  ADLS Gen2: otlp{env}adls                          │   │
│  │  (Hierarchical namespace enabled)                   │   │
│  │                                                      │   │
│  │  ├── logs/          {date}/{time}/batch.parquet     │   │
│  │  ├── traces/        {date}/{time}/batch.parquet     │   │
│  │  ├── metrics-gauge/ {date}/{time}/batch.parquet     │   │
│  │  └── metrics-sum/   {date}/{time}/batch.parquet     │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Signal Type Mapping

| Signal Type | ADLS Container | Table Name | Schema Source |
|-------------|----------------|------------|---------------|
| logs | logs | logs | otlp2records:logs |
| traces | traces | traces | otlp2records:spans |
| metrics_gauge | metrics-gauge | gauge | otlp2records:gauge |
| metrics_sum | metrics-sum | sum | otlp2records:sum |

### Deployment Strategy

**Hybrid Approach** (like AWS: CloudFormation + CLI):
- **Bicep template**: Storage account, Event Hub namespace/hub, containers
- **Azure CLI**: Stream Analytics job (inputs/outputs/query easier via CLI)

**Why Hybrid?**
- Bicep handles declarative resource creation atomically
- CLI provides better control for Stream Analytics configuration complexity
- Easier debugging and error handling during job creation
- Matches AWS pattern (CloudFormation + CLI wrappers)

### Stream Analytics Configuration

**Batching Windows** (configurable defaults):
- Time window: 5 minutes
- Size window: 2000 rows
- Balances latency vs file count

**Query** (routes by `signal_type`):
```sql
SELECT * INTO [logsoutput]
FROM [eventhubinput]
WHERE signal_type = 'logs';

SELECT * INTO [tracesoutput]
FROM [eventhubinput]
WHERE signal_type = 'traces';

SELECT * INTO [gaugeoutput]
FROM [eventhubinput]
WHERE signal_type = 'metrics_gauge';

SELECT * INTO [sumoutput]
FROM [eventhubinput]
WHERE signal_type = 'metrics_sum';
```

## CLI Commands

### Module Structure

```
src/cli/commands/azure/
├── mod.rs                      # Public API, re-exports
├── cli/                        # Service-specific CLI wrappers
│   ├── mod.rs                  # AzureCli struct
│   ├── az.rs                   # Base az command wrapper + AccountCli
│   ├── resource.rs             # ResourceCli (az group)
│   ├── storage.rs              # StorageCli (az storage)
│   ├── eventhub.rs             # EventHubCli (az eventhubs)
│   └── stream_analytics.rs     # StreamAnalyticsCli (az stream-analytics)
├── context.rs                  # DeployContext
├── create.rs                   # execute_create command
├── deploy.rs                   # Deployment helper functions
├── destroy.rs                  # execute_destroy command
├── helpers.rs                  # Name resolution, validation
├── plan.rs                     # execute_plan (dry-run)
└── status.rs                   # execute_status command
```

### Command Interface

```bash
# Initialize configuration
otlp2pipeline init --provider azure --env prod

# Create infrastructure
otlp2pipeline azure create --env prod --region westus

# Check status
otlp2pipeline azure status --env prod

# Dry-run (show what would be created)
otlp2pipeline azure plan --env prod

# Teardown
otlp2pipeline azure destroy --env prod --force
```

### Configuration File

`.otlp2pipeline.toml`:
```toml
provider = "azure"
environment = "prod"
region = "westus"                      # optional, defaults to "westus"
resource_group = "custom-rg-name"      # optional, auto-generated if not set
```

## Resource Naming

**Convention** (matches AWS pattern):

```
Environment: prod
├── Resource Group:      otlp2pipeline-prod
├── Storage Account:     otlpprodadls (no hyphens, max 24 chars)
├── Containers:          logs, traces, metrics-gauge, metrics-sum
├── Event Hub Namespace: otlp-prod-hub
├── Event Hub:           otlp-ingestion
└── Stream Analytics:    otlp-prod-stream-processor
```

**Name Resolution Logic** (`azure/helpers.rs`):

```rust
pub fn resolve_env_name(env: Option<String>) -> Result<String> {
    // Normalize: "prod" or "otlp2pipeline-prod" both resolve to "prod"
    // Matches AWS pattern in aws/helpers.rs
}

pub fn storage_account_name(env_name: &str) -> Result<String> {
    // Remove hyphens, max 24 chars, lowercase only
    // "otlp2pipeline-prod" -> "otlppipelineprodadls"
    // Truncate if needed to fit 24 char limit
}

pub fn resource_group_name(env_name: &str) -> String {
    format!("otlp2pipeline-{}", env_name)
}

pub fn eventhub_namespace(env_name: &str) -> String {
    format!("otlp-{}-hub", env_name)
}

pub fn stream_analytics_job_name(env_name: &str) -> String {
    format!("otlp-{}-stream-processor", env_name)
}
```

## Deployment Flow

### Create Command

**Phase 1: Bicep Template Deployment**
1. Create resource group if not exists
2. Deploy Bicep template:
   - Storage account (ADLS Gen2)
   - Containers: logs, traces, metrics-gauge, metrics-sum
   - Event Hub namespace + hub (4 partitions)

**Phase 2: Stream Analytics Job** (via CLI)
1. Create job: `az stream-analytics job create`
2. Configure input (Event Hub):
   - JSON serialization
   - Consumer group: $Default
3. Configure outputs (4x Parquet):
   - Output per container
   - Batching: 5 min / 2000 rows
4. Set query (signal_type routing)

**Phase 3: Start Job**
1. Start Stream Analytics job
2. Wait for "Running" state
3. Output Event Hub connection string

### Status Command

Check and report:
- ✅ Resource group exists
- ✅ Storage account + containers exist
- ✅ Event Hub namespace + hub exist
- ✅ Stream Analytics job state (Running/Stopped/Failed)
- ✅ Outputs: Event Hub connection string for example script

### Destroy Command

**Teardown Order:**
1. Stop Stream Analytics job
2. Delete Stream Analytics job
3. Delete entire resource group (includes all resources)
4. Confirmation prompt (unless `--force`)

**Resources Deleted:**
- Stream Analytics job
- Event Hub namespace + hub
- Storage account + all containers + data
- Resource group

## CLI Wrappers

### AzureCli Structure

```rust
pub struct AzureCli {
    region: String,
}

impl AzureCli {
    pub fn new(region: &str) -> Self {
        Self { region: region.to_string() }
    }

    pub fn account(&self) -> AccountCli { AccountCli::new() }
    pub fn resource(&self) -> ResourceCli { ResourceCli::new(&self.region) }
    pub fn storage(&self) -> StorageCli { StorageCli::new() }
    pub fn eventhub(&self) -> EventHubCli { EventHubCli::new() }
    pub fn stream_analytics(&self) -> StreamAnalyticsCli {
        StreamAnalyticsCli::new(&self.region)
    }
}
```

### Key Operations

**AccountCli** (`azure/cli/az.rs`):
```rust
pub fn get_subscription_id() -> Result<String>
// Runs: az account show --query id -o tsv
```

**ResourceCli** (`azure/cli/resource.rs`):
```rust
pub fn group_exists(&self, name: &str) -> Result<bool>
pub fn create_group(&self, name: &str) -> Result<()>
pub fn delete_group(&self, name: &str) -> Result<()>
```

**StorageCli** (`azure/cli/storage.rs`):
```rust
pub fn account_exists(&self, name: &str, rg: &str) -> Result<bool>
pub fn create_account(&self, name: &str, rg: &str, region: &str) -> Result<()>
pub fn container_exists(&self, container: &str, account: &str) -> Result<bool>
pub fn create_container(&self, container: &str, account: &str) -> Result<()>
pub fn get_connection_string(&self, account: &str, rg: &str) -> Result<String>
```

**EventHubCli** (`azure/cli/eventhub.rs`):
```rust
pub fn namespace_exists(&self, namespace: &str, rg: &str) -> Result<bool>
pub fn hub_exists(&self, namespace: &str, hub: &str, rg: &str) -> Result<bool>
pub fn get_connection_string(&self, namespace: &str, rg: &str) -> Result<String>
```

**StreamAnalyticsCli** (`azure/cli/stream_analytics.rs`):
```rust
pub fn job_exists(&self, job: &str, rg: &str) -> Result<bool>
pub fn create_job(&self, job: &str, rg: &str) -> Result<()>
pub fn create_input(&self, job: &str, rg: &str, config: &InputConfig) -> Result<()>
pub fn create_output(&self, job: &str, rg: &str, config: &OutputConfig) -> Result<()>
pub fn set_query(&self, job: &str, rg: &str, query: &str) -> Result<()>
pub fn start_job(&self, job: &str, rg: &str) -> Result<()>
pub fn stop_job(&self, job: &str, rg: &str) -> Result<()>
pub fn get_job_state(&self, job: &str, rg: &str) -> Result<String>
```

## DeployContext

```rust
pub struct DeployContext {
    pub subscription_id: String,
    pub env_name: String,
    pub region: String,
    pub resource_group: String,
    pub storage_account: String,
    pub eventhub_namespace: String,
    pub eventhub_name: String,
    pub stream_analytics_job: String,
    pub containers: Vec<String>, // ["logs", "traces", "metrics-gauge", "metrics-sum"]
}

impl DeployContext {
    pub fn new(cli: &AzureCli, env_name: &str, subscription_id: String) -> Result<Self> {
        // Initialize all resource names based on env_name
        // Validates name length constraints
    }
}
```

## Bicep Template

**File:** `templates/azure/otlp.bicep`

```bicep
param location string = 'westus'
param envName string
param storageAccountName string
param eventHubNamespace string

resource storageAccount 'Microsoft.Storage/storageAccounts@2023-01-01' = {
  name: storageAccountName
  location: location
  kind: 'StorageV2'
  sku: { name: 'Standard_LRS' }
  properties: {
    isHnsEnabled: true  // ADLS Gen2
  }
}

resource blobService 'Microsoft.Storage/storageAccounts/blobServices@2023-01-01' = {
  parent: storageAccount
  name: 'default'
}

resource logsContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'logs'
}

resource tracesContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'traces'
}

resource gaugeContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'metrics-gauge'
}

resource sumContainer 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = {
  parent: blobService
  name: 'metrics-sum'
}

resource eventHubNamespaceResource 'Microsoft.EventHub/namespaces@2023-01-01-preview' = {
  name: eventHubNamespace
  location: location
  sku: {
    name: 'Standard'
    tier: 'Standard'
    capacity: 1
  }
}

resource eventHub 'Microsoft.EventHub/namespaces/eventhubs@2023-01-01-preview' = {
  parent: eventHubNamespaceResource
  name: 'otlp-ingestion'
  properties: {
    partitionCount: 4
    messageRetentionInDays: 1
  }
}

output storageAccountId string = storageAccount.id
output storageAccountName string = storageAccount.name
output eventHubNamespaceId string = eventHubNamespaceResource.id
output eventHubName string = eventHub.name
```

## Example Script

**File:** `examples/azure_eventhub_poc.rs`

### Event Envelope Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub signal_type: String,  // "logs", "traces", "metrics_gauge", "metrics_sum"
    pub table: String,         // "logs", "traces", "gauge", "sum"
    pub timestamp: String,     // RFC3339
    pub service_name: Option<String>,
    pub env: Option<String>,
    pub payload: Value,        // Full otlp2records schema
}
```

### Full Schema Payloads

**Logs** (17 fields from `otlp2records:logs` schema):
```rust
fn sample_log(service_name: &str, message: &str) -> Self {
    let now = chrono::Utc::now().timestamp_millis();
    Self {
        signal_type: "logs".to_string(),
        table: "logs".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        service_name: Some(service_name.to_string()),
        env: Some("poc".to_string()),
        payload: serde_json::json!({
            // Required fields
            "timestamp": now,
            "observed_timestamp": now,
            "service_name": service_name,
            "severity_number": 9,
            "severity_text": "INFO",
            // Optional fields
            "trace_id": "0123456789abcdef0123456789abcdef",
            "span_id": "0123456789abcdef",
            "service_namespace": "poc",
            "service_instance_id": format!("{}-instance-1", service_name),
            "body": message,
            "resource_attributes": {
                "host.name": "localhost",
                "deployment.environment": "poc"
            },
            "scope_name": "azure_eventhub_poc",
            "scope_version": "0.1.0",
            "scope_attributes": {},
            "log_attributes": {
                "custom.field": "example"
            }
        }),
    }
}
```

**Traces** (27 fields from `otlp2records:spans` schema):
```rust
fn sample_trace(service_name: &str, operation: &str) -> Self {
    let now = chrono::Utc::now().timestamp_millis();
    let duration_us = 125_000; // 125ms
    Self {
        signal_type: "traces".to_string(),
        table: "traces".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        service_name: Some(service_name.to_string()),
        env: Some("poc".to_string()),
        payload: serde_json::json!({
            // Required fields
            "timestamp": now,
            "end_timestamp": now + 125,
            "duration": duration_us,
            "service_name": service_name,
            "span_name": operation,
            "span_kind": 1, // INTERNAL
            "status_code": 1, // OK
            // Optional fields
            "trace_id": "0123456789abcdef0123456789abcdef",
            "span_id": "0123456789abcdef",
            "parent_span_id": "fedcba9876543210",
            "trace_state": "",
            "service_namespace": "poc",
            "service_instance_id": format!("{}-instance-1", service_name),
            "status_message": "",
            "resource_attributes": {
                "host.name": "localhost",
                "deployment.environment": "poc"
            },
            "scope_name": "azure_eventhub_poc",
            "scope_version": "0.1.0",
            "scope_attributes": {},
            "span_attributes": {
                "http.method": "GET",
                "http.status_code": 200
            },
            "events_json": [],
            "links_json": [],
            "dropped_attributes_count": 0,
            "dropped_events_count": 0,
            "dropped_links_count": 0,
            "flags": 0
        }),
    }
}
```

**Gauge** (18 fields from `otlp2records:gauge` schema):
```rust
fn sample_gauge(service_name: &str, metric_name: &str, value: f64) -> Self {
    let now = chrono::Utc::now().timestamp_millis();
    Self {
        signal_type: "metrics_gauge".to_string(),
        table: "gauge".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        service_name: Some(service_name.to_string()),
        env: Some("poc".to_string()),
        payload: serde_json::json!({
            // Required fields
            "timestamp": now,
            "metric_name": metric_name,
            "value": value,
            "service_name": service_name,
            // Optional fields
            "start_timestamp": now - 60000,
            "metric_description": format!("Gauge metric: {}", metric_name),
            "metric_unit": "bytes",
            "service_namespace": "poc",
            "service_instance_id": format!("{}-instance-1", service_name),
            "resource_attributes": {
                "host.name": "localhost"
            },
            "scope_name": "azure_eventhub_poc",
            "scope_version": "0.1.0",
            "scope_attributes": {},
            "metric_attributes": {
                "environment": "poc"
            },
            "flags": 0,
            "exemplars_json": []
        }),
    }
}
```

**Sum** (20 fields from `otlp2records:sum` schema):
```rust
fn sample_sum(service_name: &str, metric_name: &str, value: f64) -> Self {
    let now = chrono::Utc::now().timestamp_millis();
    Self {
        signal_type: "metrics_sum".to_string(),
        table: "sum".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        service_name: Some(service_name.to_string()),
        env: Some("poc".to_string()),
        payload: serde_json::json!({
            // Required fields
            "timestamp": now,
            "metric_name": metric_name,
            "value": value,
            "service_name": service_name,
            "aggregation_temporality": 2, // DELTA
            "is_monotonic": true,
            // Optional fields
            "start_timestamp": now - 60000,
            "metric_description": format!("Sum metric: {}", metric_name),
            "metric_unit": "count",
            "service_namespace": "poc",
            "service_instance_id": format!("{}-instance-1", service_name),
            "resource_attributes": {
                "host.name": "localhost"
            },
            "scope_name": "azure_eventhub_poc",
            "scope_version": "0.1.0",
            "scope_attributes": {},
            "metric_attributes": {
                "environment": "poc"
            },
            "flags": 0,
            "exemplars_json": []
        }),
    }
}
```

### Usage

```bash
# Set connection string
export EVENTHUB_CONNECTION_STRING="Endpoint=sb://otlp-prod-hub.servicebus.windows.net/;..."

# Run example
cargo run --example azure_eventhub_poc --features azure
```

## Error Handling

### Validation

**Pre-flight Checks** (before deployment):
1. Azure CLI installed: `az --version`
2. Logged in: `az account show`
3. Subscription ID available
4. Name length constraints:
   - Storage account ≤ 24 chars (lowercase, no hyphens)
   - Resource group ≤ 90 chars
5. Region availability

### Error Strategy

1. **CLI Wrapper Errors**: All `az` commands return `Result<T, anyhow::Error>`
2. **Retry Logic**: None initially (Azure CLI handles retries internally)
3. **Rollback**: Manual via `destroy` command (like AWS)
4. **Partial Failures**: Report which phase failed, suggest manual cleanup

### Common Error Scenarios

**Storage account name invalid:**
```
Error: Storage account name 'otlp-pipeline-prod-adls' is invalid
Solution: Name must be 3-24 chars, lowercase, no hyphens
Generated name: 'otlppipelineprodadls'
```

**Azure CLI not logged in:**
```
Error: Not logged in to Azure
Solution: Run 'az login' to authenticate
```

**Stream Analytics job failed to start:**
```
Error: Stream Analytics job failed to start
State: Failed
Solution: Check Azure Portal for detailed error logs
```

## Authentication

**Method:** Azure CLI authentication (matches AWS pattern)

**Requirements:**
1. User runs `az login` once
2. CLI reads credentials from `~/.azure/` directory
3. Subscription ID auto-detected or set via `az account set`

**No explicit credentials in environment variables** (for initial implementation).

## Implementation Order

### Phase 1: Foundation (~4 files)
1. `azure/cli/az.rs` - Base command runner + AccountCli
2. `azure/cli/resource.rs` - Resource group operations
3. `azure/helpers.rs` - Name resolution, validation
4. `azure/context.rs` - DeployContext struct
5. Update `src/cli/commands/mod.rs` to include azure module
6. Update CLI args in `src/cli/mod.rs` to support azure provider

### Phase 2: Storage & Event Hub (~3 files)
7. `templates/azure/otlp.bicep` - Bicep template
8. `azure/cli/storage.rs` - Storage account + container operations
9. `azure/cli/eventhub.rs` - Event Hub operations

### Phase 3: Stream Analytics (~2 files)
10. `azure/cli/stream_analytics.rs` - Complete Stream Analytics operations
11. `azure/deploy.rs` - Deployment orchestration functions

### Phase 4: Commands (~4 files)
12. `azure/create.rs` - Main create command
13. `azure/status.rs` - Status checking
14. `azure/destroy.rs` - Teardown
15. `azure/plan.rs` - Dry-run

### Phase 5: Example Script (~1 file)
16. Update `examples/azure_eventhub_poc.rs` - Add full schemas + metrics_sum

## Testing

### Manual Testing

```bash
# 1. Create infrastructure
otlp2pipeline azure create --env test --region westus

# 2. Check status
otlp2pipeline azure status --env test

# 3. Run example script
export EVENTHUB_CONNECTION_STRING="..."
cargo run --example azure_eventhub_poc --features azure

# 4. Wait 5 minutes for Stream Analytics to flush batches

# 5. Verify Parquet files in ADLS
az storage blob list \
  --container-name logs \
  --account-name otlptestadls \
  --auth-mode login

# 6. Download and inspect Parquet
az storage blob download \
  --container-name logs \
  --account-name otlptestadls \
  --name "2026/01/14/04/batch.parquet" \
  --file logs.parquet

# 7. Teardown
otlp2pipeline azure destroy --env test --force
```

### Validation Checklist

- [ ] All resources created successfully
- [ ] Stream Analytics job starts and reaches "Running" state
- [ ] Example script sends 60+ events (logs, traces, gauge, sum)
- [ ] Parquet files appear in correct containers after 5 min
- [ ] Each container has correct signal_type events only
- [ ] Logs container has only `signal_type = 'logs'`
- [ ] Traces container has only `signal_type = 'traces'`
- [ ] Metrics-gauge container has only `signal_type = 'metrics_gauge'`
- [ ] Metrics-sum container has only `signal_type = 'metrics_sum'`
- [ ] Parquet schema matches otlp2records (verify with pyarrow)
- [ ] Destroy removes all resources cleanly

## Success Criteria

Initial implementation is successful when:

1. ✅ CLI commands work: `create`, `status`, `destroy`, `plan`
2. ✅ Resources deploy correctly via Bicep + CLI
3. ✅ Stream Analytics routes events by `signal_type`
4. ✅ Example script posts data matching full otlp2records schemas
5. ✅ Parquet files written to ADLS Gen2 with correct data
6. ✅ Clean teardown with `destroy` command
7. ✅ Consistent with AWS command patterns

## Future Enhancements

**Not in initial scope:**

1. **Azure Function Deployment**
   - HTTP trigger receiving OTLP requests
   - Full transformation pipeline (OTLP → JSON → Event Hub)
   - Matches AWS Lambda deployment pattern

2. **Query Support**
   - Azure Data Explorer integration
   - Fabric Lakehouse integration
   - Direct Parquet queries via `otlp2pipeline azure query`

3. **Catalog Operations**
   - List tables: `otlp2pipeline azure catalog list`
   - Show schema: `otlp2pipeline azure catalog schema logs`

4. **Additional Signal Types**
   - Histogram metrics support
   - Exponential histogram support

5. **Advanced Stream Analytics**
   - Custom windowing configurations
   - Aggregations before Parquet write
   - Multiple Event Hub inputs

## References

- [Azure Event Hubs documentation](https://learn.microsoft.com/azure/event-hubs/)
- [Azure Stream Analytics documentation](https://learn.microsoft.com/azure/stream-analytics/)
- [ADLS Gen2 documentation](https://learn.microsoft.com/azure/storage/blobs/data-lake-storage-introduction)
- [Bicep documentation](https://learn.microsoft.com/azure/azure-resource-manager/bicep/)
- [otlp2records crate](https://crates.io/crates/otlp2records)
- [Azure PoC Guide](../azure-poc-guide.md)
