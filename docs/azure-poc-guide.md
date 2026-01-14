# Azure Event Hub + Stream Analytics PoC Guide

This guide walks through running the proof-of-concept (PoC) to validate Azure Event Hubs integration with Stream Analytics for automatic Parquet batching and signal-type routing.

## Goals

Validate that:
1. ✅ Rust can connect to Azure Event Hub using the `azeventhubs` SDK
2. ✅ Event envelope schema works for telemetry data
3. ✅ Events are successfully received by Event Hub
4. ✅ Stream Analytics routes events by `signal_type` to separate outputs
5. ✅ Parquet files are automatically batched and written to ADLS Gen2
6. ✅ Batch sending and error handling work as expected

## Prerequisites

### 1. Azure Account Setup

- Azure subscription (personal or organizational)
- Ability to create Event Hubs, Storage accounts, and Stream Analytics jobs

### 2. Azure Data Lake Storage Gen2 Setup

Create a storage account with hierarchical namespace enabled:

```bash
# Create resource group (if not exists)
az group create \
  --name fabrictest01 \
  --location westus

# Create ADLS Gen2 storage account
az storage account create \
  --name otlppocadls \
  --resource-group fabrictest01 \
  --location westus \
  --sku Standard_LRS \
  --kind StorageV2 \
  --enable-hierarchical-namespace true

# Create containers for each signal type
az storage container create \
  --name logs \
  --account-name otlppocadls \
  --auth-mode login

az storage container create \
  --name traces \
  --account-name otlppocadls \
  --auth-mode login

az storage container create \
  --name metrics \
  --account-name otlppocadls \
  --auth-mode login
```

### 3. Azure Event Hubs Setup

Create Event Hubs namespace and hub (no Capture needed):

```bash
# Create Event Hubs namespace
az eventhubs namespace create \
  --resource-group fabrictest01 \
  --name otlp-poc-hub \
  --location westus \
  --sku Standard

# Create Event Hub
az eventhubs eventhub create \
  --resource-group fabrictest01 \
  --namespace-name otlp-poc-hub \
  --name otlp-ingestion \
  --partition-count 4

# Get connection string
az eventhubs namespace authorization-rule keys list \
  --resource-group fabrictest01 \
  --namespace-name otlp-poc-hub \
  --name RootManageSharedAccessKey \
  --query primaryConnectionString -o tsv
```

### 4. Azure Stream Analytics Job Setup

Create Stream Analytics job to route events:

```bash
# Create Stream Analytics job
az stream-analytics job create \
  --resource-group fabrictest01 \
  --name otlp-stream-processor \
  --location westus \
  --streaming-units 1 \
  --output-error-policy Drop \
  --events-out-of-order-policy Adjust \
  --events-out-of-order-max-delay 10 \
  --events-late-arrival-max-delay 5
```

**Note:** We'll configure inputs, outputs, and query via Azure Portal (easier than CLI for complex setups).

### 5. Local Development Setup

Install Rust and required tools:

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify installation
cargo --version
```

## Running the PoC

### Step 1: Configure Stream Analytics Job (Azure Portal)

Navigate to Azure Portal → Stream Analytics → `otlp-stream-processor`

#### A. Configure Input (Event Hub)

1. Click **Inputs** → **+ Add stream input** → **Event Hub**
2. Configure:
   - **Input alias**: `eventhub_input`
   - **Subscription**: Select your subscription
   - **Event Hub namespace**: `otlp-poc-hub`
   - **Event Hub name**: `otlp-ingestion`
   - **Consumer group**: `$Default`
   - **Authentication mode**: Connection string
   - **Event serialization format**: JSON
   - **Encoding**: UTF-8
3. Click **Save**

#### B. Configure Outputs (3x ADLS Gen2 Parquet)

**Output 1: Logs**

1. Click **Outputs** → **+ Add** → **Blob storage/ADLS Gen2**
2. Configure:
   - **Output alias**: `logs_output`
   - **Subscription**: Select your subscription
   - **Storage account**: `otlppocadls`
   - **Container**: `logs`
   - **Path pattern**: `{date}/{time}` (creates hierarchy)
   - **Date format**: YYYY/MM/DD
   - **Time format**: HH
   - **Event serialization format**: **Parquet**
   - **Minimum rows**: `2000` (batching size window)
   - **Maximum time**: `00:05:00` (5 minute batching time window)
   - **Authentication mode**: Connection string
3. Click **Save**

**Output 2: Traces**

Repeat above with:
- **Output alias**: `traces_output`
- **Container**: `traces`
- Same Parquet settings

**Output 3: Metrics**

Repeat above with:
- **Output alias**: `metrics_output`
- **Container**: `metrics`
- Same Parquet settings

#### C. Configure Query (Signal Routing)

1. Click **Query**
2. Replace with:

```sql
-- Route logs by signal_type
SELECT
    *
INTO
    [logs_output]
FROM
    [eventhub_input]
WHERE
    signal_type = 'logs'

-- Route traces by signal_type
SELECT
    *
INTO
    [traces_output]
FROM
    [eventhub_input]
WHERE
    signal_type = 'traces'

-- Route metrics (all types: gauge, sum, histogram)
SELECT
    *
INTO
    [metrics_output]
FROM
    [eventhub_input]
WHERE
    signal_type LIKE 'metrics_%'
```

3. Click **Save query**
4. Click **Test query** (optional - use sample data)

#### D. Start the Job

1. Click **Overview**
2. Click **Start**
3. Job output start time: **Now**
4. Click **Start**
5. Wait ~1-2 minutes for job to enter "Running" state

### Step 2: Set Environment Variables

```bash
# Required: Event Hub connection string (from Step 3)
export EVENTHUB_CONNECTION_STRING="Endpoint=sb://otlp-poc-hub.servicebus.windows.net/;SharedAccessKeyName=RootManageSharedAccessKey;SharedAccessKey=YOUR_KEY_HERE"

# Optional: Event Hub name (defaults to "otlp-ingestion")
export EVENTHUB_NAME="otlp-ingestion"

# Optional: Enable debug logging
export RUST_LOG=info
```

**Security Note:** Never commit connection strings to version control.

### Step 3: Build and Run PoC

From the project root:

```bash
# Build the PoC example
cargo build --example azure_eventhub_poc --features azure

# Run the PoC
cargo run --example azure_eventhub_poc --features azure
```

Expected output:

```
🚀 Azure Event Hub PoC - Starting

📋 Configuration:
   Event Hub Name: otlp-ingestion
   Connection String: Endpoint=sb://otlp-poc-hub.servicebus.windows...

🔌 Creating Event Hub producer client...
✅ Producer client created successfully

📤 Test 1: Sending single log event
✅ Log event sent successfully
   Signal Type: logs
   Service: api-gateway
   Payload size: 300 bytes

📤 Test 2: Sending batch of 5 trace events
✅ Batch of 5 trace events sent successfully

📤 Test 3: Sending mixed signal types (logs, traces, metrics)
   ✓ Sent logs event from web-server
   ✓ Sent traces event from web-server
   ✓ Sent metrics_gauge event from web-server
   ✓ Sent logs event from database
   ✓ Sent traces event from database
✅ Mixed events sent successfully

📤 Test 4: Sending large batch (50 events)
   ✓ Sent 10 events...
   ✓ Sent 20 events...
   ✓ Sent 30 events...
   ✓ Sent 40 events...
   ✓ Sent 50 events...
✅ Large batch sent successfully

📋 Test 5: Verifying envelope schema
✅ Schema verified

🎉 PoC Complete!

Summary:
  ✓ Single event send
  ✓ Batch event send
  ✓ Mixed signal types
  ✓ Large batch (50 events)
  ✓ Schema validation

Next steps:
  1. Wait 5 minutes for Stream Analytics to flush batches
  2. Check ADLS Gen2 containers for Parquet files
  3. Download and inspect Parquet data
  4. Verify signal_type routing worked
```

### Step 4: Monitor Stream Analytics Job

Check job metrics in Azure Portal:

1. Navigate to Stream Analytics → `otlp-stream-processor` → **Monitoring**
2. Metrics to check:
   - **Input Events**: Should show ~61 events
   - **Output Events**: Should show ~61 total across all outputs
   - **Data Conversion Errors**: Should be 0
   - **Runtime Errors**: Should be 0

### Step 5: Verify Parquet Files in ADLS

Wait ~5 minutes after running PoC (time window = 5 minutes), then check:

```bash
# Check logs container
az storage blob list \
  --container-name logs \
  --account-name otlppocadls \
  --auth-mode key \
  --query "[?ends_with(name, '.parquet')].[name, properties.contentLength]" \
  --output table

# Check traces container
az storage blob list \
  --container-name traces \
  --account-name otlppocadls \
  --auth-mode key \
  --query "[?ends_with(name, '.parquet')].[name, properties.contentLength]" \
  --output table

# Check metrics container
az storage blob list \
  --container-name metrics \
  --account-name otlppocadls \
  --auth-mode key \
  --query "[?ends_with(name, '.parquet')].[name, properties.contentLength]" \
  --output table
```

Expected output structure:

```
logs/
  2026/01/14/04/
    <timestamp>.parquet  (~X KB - contains log events)

traces/
  2026/01/14/04/
    <timestamp>.parquet  (~X KB - contains trace events)

metrics/
  2026/01/14/04/
    <timestamp>.parquet  (~X KB - contains metrics events)
```

### Step 6: Download and Inspect Parquet Files

```bash
# Download logs Parquet file
az storage blob download \
  --container-name logs \
  --account-name otlppocadls \
  --name "2026/01/14/04/<timestamp>.parquet" \
  --file logs.parquet \
  --auth-mode key

# Inspect with Python/PyArrow
uvx --with pyarrow --with pandas python3 << 'EOF'
import pandas as pd
import pyarrow.parquet as pq

# Read Parquet file
df = pd.read_parquet('logs.parquet')

print(f"✅ Found {len(df)} log records")
print("\n📋 Schema:")
print(df.dtypes)
print("\n📊 Sample records:")
print(df.head())
print("\n🔍 Signal type distribution:")
print(df['signal_type'].value_counts())
EOF
```

Expected Parquet schema:

```
signal_type        object
table              object
timestamp          object
service_name       object
env                object
payload            object  (nested JSON)
EnqueuedTimeUtc    object
SequenceNumber     int64
Offset             object
```

**Validation Checklist:**
- [ ] Parquet files exist in each container (logs, traces, metrics)
- [ ] File paths follow date/time hierarchy
- [ ] Each file contains only its signal_type (logs → logs only)
- [ ] `signal_type` field matches container name
- [ ] `payload` contains the transformed JSON record
- [ ] All 61 events accounted for across containers

## Troubleshooting

### Error: "Connection refused" or "Unauthorized"

**Cause:** Invalid Event Hub connection string

**Solution:**
1. Verify connection string format:
   ```
   Endpoint=sb://<namespace>.servicebus.windows.net/;SharedAccessKeyName=...;SharedAccessKey=...
   ```
2. Check firewall rules (Event Hubs → Networking)
3. Verify SAS key hasn't expired

### Stream Analytics job showing "Degraded" or errors

**Cause:** Configuration issues with inputs/outputs

**Solution:**
1. Check **Operation logs** in Stream Analytics job
2. Verify storage account connection string is valid
3. Ensure containers exist (`logs`, `traces`, `metrics`)
4. Test input/output connections in Portal

### No Parquet files appearing

**Cause:** Batch window not elapsed or no events matching filter

**Solution:**
1. Wait full 5 minutes (time window) after sending events
2. Check Stream Analytics metrics:
   - **Input Events** > 0
   - **Output Events** > 0
   - **Data Conversion Errors** = 0
3. Verify query syntax is correct (no SQL errors)
4. Check if events have correct `signal_type` field

### Parquet files empty or corrupted

**Cause:** JSON parsing issues or serialization errors

**Solution:**
1. Check Stream Analytics **Data Conversion Errors** metric
2. Verify events are valid JSON:
   ```bash
   echo '{"signal_type":"logs","table":"logs","timestamp":"2026-01-14T00:00:00Z","payload":{}}' | jq
   ```
3. Inspect job logs in Portal for detailed errors
4. Test query with sample data in Query editor

### Events going to wrong container

**Cause:** Incorrect query filters or missing `signal_type` field

**Solution:**
1. Verify `signal_type` field exists in all events
2. Check query WHERE clauses match exactly:
   - `signal_type = 'logs'` (not `'log'`)
   - `signal_type = 'traces'` (not `'trace'`)
   - `signal_type LIKE 'metrics_%'` (matches gauge, sum, histogram)
3. Use Stream Analytics diagnostics logs to see routing

## Success Criteria

The PoC is successful if:

- [x] PoC binary compiles with `--features azure`
- [ ] PoC connects to Event Hub without errors
- [ ] All 61 events sent successfully (no send failures)
- [ ] Stream Analytics job runs without errors
- [ ] Parquet files appear in ADLS within 5 minutes
- [ ] **Signal routing works:** logs → logs/, traces → traces/, metrics → metrics/
- [ ] Parquet files contain expected JSON structure
- [ ] `signal_type` and `payload` fields are preserved
- [ ] Event counts match (61 total = logs + traces + metrics)

## Next Steps After Successful PoC

### 1. Production Configuration

**Optimize Batching:**
- Adjust `timeWindow` based on latency requirements (1-10 minutes)
- Adjust `sizeWindow` based on throughput (2,000-10,000 rows)
- Smaller batches = lower latency, more files
- Larger batches = higher latency, fewer files, lower cost

**Scale Stream Analytics:**
- Start with 1 SU (Streaming Unit) for PoC
- Production: 3-6 SUs for high throughput
- Use autoscaling if available

**Partition Strategy:**
- Path pattern: `{date}/{time}/{signal_type}` for time-based queries
- Or: `{signal_type}/{date}/{time}` for signal-type filtering

### 2. Proceed with Implementation

Create production Rust service:
- `src/azure/eventhub.rs` - EventHub sender implementation
- `src/bin/azure_fn.rs` - Azure Function entry point (HTTP trigger)
- Deploy as Azure Function with Event Hub binding
- Add retry logic, circuit breakers, metrics

### 3. Delta Lake Conversion (Optional)

Convert Parquet → Delta Lake for ACID transactions:

```bash
# Use Delta Standalone or Databricks
# Read Parquet files from ADLS
# Convert to Delta format
# Benefits: time travel, schema evolution, ACID
```

### 4. Query with DuckDB or Fabric

Query Parquet files directly:

```sql
-- DuckDB
SELECT signal_type, COUNT(*)
FROM 'logs/*.parquet'
GROUP BY signal_type;

-- Fabric Lakehouse
CREATE EXTERNAL TABLE logs_table
LOCATION 'abfss://logs@otlppocadls.dfs.core.windows.net/'
FILE_FORMAT = PARQUET;
```

### 5. Update Design Document

Document learnings from PoC:
- Event Hub throughput observed
- Stream Analytics latency (event → Parquet write)
- Parquet file sizes and batch counts
- Cost estimates (Event Hub TUs, Stream Analytics SUs, ADLS storage)

### 6. Cleanup PoC Resources

```bash
# Stop Stream Analytics job
az stream-analytics job stop \
  --resource-group fabrictest01 \
  --name otlp-stream-processor

# Delete entire resource group (optional)
az group delete --name fabrictest01 --yes
```

## PoC Validation Report Template

After completing the PoC, fill out this report:

```markdown
## Azure Event Hub + Stream Analytics PoC Results

**Date:** YYYY-MM-DD
**Tester:** [Your Name]
**Environment:** [Azure subscription ID]

### Test Results

| Test | Status | Notes |
|------|--------|-------|
| SDK Connection | ✅/❌ | |
| Single Event Send | ✅/❌ | |
| Batch Send (5 events) | ✅/❌ | |
| Mixed Signal Types | ✅/❌ | |
| Large Batch (50 events) | ✅/❌ | |
| Schema Validation | ✅/❌ | |
| Stream Analytics Job Start | ✅/❌ | Time to Running: X min |
| Logs Routing | ✅/❌ | Events: X |
| Traces Routing | ✅/❌ | Events: X |
| Metrics Routing | ✅/❌ | Events: X |
| Parquet Write | ✅/❌ | Files: X |
| Parquet Inspection | ✅/❌ | Records validated: X |

### Observations

**Performance:**
- Average send latency: XXX ms
- Stream Analytics processing latency: XXX seconds
- Time to first Parquet file: XXX minutes

**Storage:**
- Parquet file sizes: XXX KB per file
- Batch counts: XXX events per file
- File path structure: [verified/issues noted]

**Schema:**
- JSON envelope preserved: ✅/❌
- Signal routing accuracy: XX/61 events correct
- Parquet schema valid: ✅/❌

**Issues Encountered:**
- [List any problems and resolutions]

### Recommendation

[ ] Proceed with Stream Analytics for production
[ ] Requires adjustments (specify below)
[ ] Block - critical issues found

**Rationale:**
[Explain recommendation]

**Production Settings:**
- Streaming Units: X SUs
- Batch time window: X minutes
- Batch size window: X rows
- Estimated cost: $X/month
```

## Architecture Comparison

| Approach | Pros | Cons | Best For |
|----------|------|------|----------|
| **Event Hubs Capture** | - Simple setup<br>- No routing needed | - No signal routing<br>- Avro format (needs conversion) | Single stream, post-processing |
| **Stream Analytics** ✅ | - Signal routing built-in<br>- Native Parquet<br>- Managed service | - Additional cost<br>- ~1 min latency | Multi-signal routing, Parquet output |
| **Custom Consumer** | - Full control<br>- Write to Delta directly | - Code to maintain<br>- Need to deploy | Complex transformations, Delta Lake |

## Additional Resources

- [Azure Stream Analytics documentation](https://learn.microsoft.com/azure/stream-analytics/)
- [Stream Analytics Parquet output](https://learn.microsoft.com/azure/stream-analytics/stream-analytics-define-outputs)
- [Azure Event Hubs documentation](https://learn.microsoft.com/azure/event-hubs/)
- [azeventhubs Rust crate](https://docs.rs/azeventhubs/)
- [Apache Parquet specification](https://parquet.apache.org/)
- [DuckDB Parquet support](https://duckdb.org/docs/data/parquet)
