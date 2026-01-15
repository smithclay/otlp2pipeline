# Azure Integration PoC - Quick Start

This directory contains the proof-of-concept implementation for Azure Function + Fabric Eventstream integration.

## What Was Created

### 1. Cargo.toml Updates
- ✅ Added `azure` feature flag
- ✅ Added Azure SDK dependencies:
  - `azure_messaging_event_hubs` v0.22
  - `azure_identity` v0.22
  - `azure_core` v0.22

### 2. PoC Binary (`examples/azure_eventhub_poc.rs`)
A standalone test program that validates:
- Event Hub connection using Azure SDK
- Event envelope schema for Fabric routing
- Batch sending (single, small batch, large batch)
- Mixed signal types (logs, traces, metrics)
- Error handling and retry readiness

### 3. Documentation
- **`azure-poc-guide.md`**: Complete step-by-step testing guide
- **`2026-01-12-azure-fabric-brainstorm.md`**: Detailed architecture design

## Quick Start

### Prerequisites
1. Azure account with Event Hubs or Fabric Eventstream access
2. Event Hub connection string
3. Rust toolchain installed

### Run PoC in 3 Steps

```bash
# 1. Set connection string
export EVENTHUB_CONNECTION_STRING="Endpoint=sb://..."
export EVENTHUB_NAME="otlp-ingestion"

# 2. Build and run
cargo run --example azure_eventhub_poc --features azure

# 3. Verify in Azure Portal
# Check Event Hub metrics for ~61 incoming messages
```

### Expected Output

```
🚀 Azure Event Hub PoC - Starting
...
🎉 PoC Complete!

Summary:
  ✓ Single event send
  ✓ Batch event send
  ✓ Mixed signal types
  ✓ Large batch (50 events)
  ✓ Schema validation
```

## Event Envelope Schema

The PoC validates this envelope structure:

```json
{
  "signal_type": "logs",          // Fabric routing key
  "table": "logs",                // Target Delta table
  "timestamp": "2026-01-12T10:30:00Z",
  "service_name": "api-gateway",  // Optional partition key
  "env": "poc",                   // Optional tenant/environment
  "payload": {
    // Transformed JSON record (output from VRL)
    "timestamp": 1736682600000,
    "service_name": "api-gateway",
    "body": "Request received",
    // ... rest of signal-specific fields
  }
}
```

## What Happens Next?

### If PoC Succeeds ✅

1. Proceed with full implementation (see `azure-fabric-brainstorm.md`)
2. Create `src/azure/eventstream.rs` with `EventHubProducerSender`
3. Create `src/bin/azure_fn.rs` Azure Function entry point
4. Implement retry logic and PipelineSender trait
5. Add CLI commands (`azure create`, `azure status`)

### If PoC Fails ❌

Common issues and fixes:
- **Connection errors:** Verify connection string format and network access
- **Compilation errors:** Install OpenSSL dev libraries
- **Events not received:** Check Event Hub name and firewall rules
- **Schema issues:** Review envelope structure in Data Preview

See `azure-poc-guide.md` for detailed troubleshooting.

## Files Created

```
otlp2pipeline/
├── Cargo.toml                      # Updated with azure feature
├── examples/
│   └── azure_eventhub_poc.rs       # PoC binary
└── docs/
    ├── azure-poc-README.md         # This file
    ├── azure-poc-guide.md          # Detailed testing guide
    └── plans/
        └── 2026-01-12-azure-fabric-brainstorm.md  # Architecture design
```

## Architecture Summary

```
┌─────────────────┐
│ OTLP Client     │
└────────┬────────┘
         │ POST /v1/logs
         ▼
┌─────────────────────────────────────┐
│ Azure Function (Container)          │
│  ├─ handle_signal<LogsHandler>     │ ◄── Shared with Lambda/WASM
│  ├─ VRL Transform                   │
│  └─ EventHubProducerSender          │
└────────┬────────────────────────────┘
         │ JSON envelope with signal_type
         ▼
┌─────────────────────────────────────┐
│ Fabric Eventstream (Custom Endpoint)│
│  ├─ Derived Stream: logs           │
│  ├─ Derived Stream: traces         │
│  └─ Derived Stream: metrics        │
└────────┬────────────────────────────┘
         │ Routed by signal_type
         ▼
┌─────────────────────────────────────┐
│ OneLake Delta Tables                │
│  ├─ bronze_logs_json                │
│  ├─ bronze_traces_json              │
│  ├─ bronze_gauge_json               │
│  └─ bronze_sum_json                 │
└─────────────────────────────────────┘
```

## Key Differences from AWS Lambda

| Aspect | AWS Lambda | Azure Function (PoC) |
|--------|------------|----------------------|
| Entry | Function URL | HTTP Trigger |
| SDK | `aws-sdk-firehose` | `azure_messaging_event_hubs` |
| Batch Limit | 500 records | 100 events |
| Routing | 4 Firehose streams | 1 Eventstream + Fabric filters |
| Storage | S3 Tables (Iceberg) | OneLake (Delta Lake) |
| Envelope | Direct JSON | Wrapped with signal_type |

## Success Criteria

The PoC validates these assumptions:

- [x] Azure SDK compiles and links correctly
- [ ] Connection to Event Hub succeeds
- [ ] Events are sent without errors
- [ ] Envelope schema is valid JSON
- [ ] Fabric Eventstream receives events
- [ ] Derived streams can route by signal_type
- [ ] Delta Lake tables accept payload structure

## Support and Issues

For PoC issues:
1. Check `azure-poc-guide.md` troubleshooting section
2. Review Azure SDK documentation
3. Verify Event Hub configuration in Azure Portal

For design questions:
- See `2026-01-12-azure-fabric-brainstorm.md`

## Next Steps

1. **Run the PoC:**
   ```bash
   cargo run --example azure_eventhub_poc --features azure
   ```

2. **Verify in Fabric Eventstream:**
   - Check metrics for incoming messages
   - Configure derived streams
   - Query Delta Lake tables

3. **Fill out validation report** (template in `azure-poc-guide.md`)

4. **Proceed with implementation** if all criteria met
