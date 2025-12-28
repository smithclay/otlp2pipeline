# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build for WASM (Cloudflare Workers)
cargo build --target wasm32-unknown-unknown --release

# Build for native (tests)
cargo build

# Run all tests
cargo test

# Run specific test file
cargo test --test e2e_logs
cargo test --test e2e_traces

# Check WASM bundle size
./scripts/check-size.sh

# Deploy to Cloudflare
npx wrangler deploy

# Local development (note: secrets not available without .dev.vars)
npx wrangler dev
```

## Architecture

This is an OTLP (OpenTelemetry Protocol) ingestion worker that receives telemetry data (logs, traces) and forwards it to Cloudflare Pipelines for storage in R2/Iceberg.

### Dual-Target Build

The crate builds for two targets via `#[cfg]` attributes:
- **WASM** (`wasm32-unknown-unknown`): Cloudflare Worker using `worker` crate
- **Native**: Axum server using `reqwest` (for testing)

Entry points are in `src/lib.rs`:
- WASM: `mod wasm` with `#[event(fetch)]`
- Native: `mod native` with Axum router

### Request Flow

```
HTTP POST /v1/{logs,traces}
    → parse_content_metadata (gzip?, protobuf/json?)
    → handle_signal<H: SignalHandler>
        → decompress_if_gzipped
        → H::decode (OTLP → VRL Values)
        → VrlTransformer::transform_batch (run VRL program)
        → PipelineSender::send_all (forward to Cloudflare Pipeline)
```

### SignalHandler Trait

Each telemetry signal type (logs, traces) implements `SignalHandler` in `src/handler.rs`:
- `Signal` enum in `src/signal.rs` defines all supported types
- Handler provides decode function and VRL program reference
- Generic `handle_signal<H>` processes any handler type

### OTLP Decoding (`src/decode/otlp/`)

Parallel structure for each signal:
- `logs.rs` / `traces.rs`: Format routing (protobuf vs JSON)
- `logs_proto.rs` / `traces_proto.rs`: Protobuf via `opentelemetry-proto` crate
- `logs_json.rs` / `traces_json.rs`: JSON via serde
- `common.rs`: Shared utilities (DecodeError, JSON structs, timestamp conversion)
- `record_builder/`: Builder pattern for record construction (split into log, span, metric builders)

### VRL Transformation (`src/transform/`)

VRL scripts in `vrl/*.vrl` are compiled at build time (`build.rs`):
- `otlp_logs.vrl`: Flatten log records (16 fields)
- `otlp_traces.vrl`: Flatten span records (26 fields)

Custom VRL functions in `src/transform/functions.rs` (minimal set for WASM compatibility).

Scripts assign:
- `._table` to route records to the correct pipeline
- `._signal` for deterministic sorting ("logs" or "traces")

### Schema Unification (`build.rs`)

VRL `# @schema` comments are the **single source of truth** for all schemas. The build script parses these annotations and generates:

- `schemas/*.schema.json` - Cloudflare Pipeline schemas
- `$OUT_DIR/arrow_schemas.rs` - Arrow schema functions (`logs_schema()`, `spans_schema()`)
- `$OUT_DIR/sqlite_ddl.rs` - SQLite CREATE TABLE statements (`LOGS_DDL`, `TRACES_DDL`)
- `$OUT_DIR/insert_helpers.rs` - Type-safe insert helpers (`logs_insert_sql()`, `logs_values()`)

Schema field types: `timestamp`, `int64`, `int32`, `float64`, `bool`, `string`, `json`

Timestamps are stored as **milliseconds** (Int64).

### Hot Cache (`src/cache/`)

Durable Objects provide low-latency access to recent telemetry via SQLite:

- `durable_object.rs`: `HotCacheDO` with SQLite storage per {service}:{signal}
- `arrow_convert.rs`: JSON → Arrow RecordBatch conversion (uses generated schemas)
- `insert_helpers.rs`: Wraps generated insert helpers with transaction batching
- `query.rs`: Query API with fanout to multiple DOs
- `sender.rs`: Routes records to appropriate DO instances

Query responses support JSON and Arrow IPC formats via content negotiation.

### Pipeline Client (`src/pipeline/`)

- `client.rs`: Multi-signal client with per-signal endpoints
- `sender.rs`: `PipelineSender` trait for abstraction
- `retry.rs`: Retry with exponential backoff

WASM uses `worker::Fetch`, native uses `reqwest`.

## Testing

E2E tests use an in-process Axum mock server (`tests/helpers/mod.rs`) that:
1. Receives JSON payloads from the native Axum server
2. Stores events for validation

Tests spawn the mock server in-memory and clean up automatically.
