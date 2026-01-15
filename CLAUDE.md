# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build for WASM (Cloudflare Workers) - use --lib to skip CLI binary
cargo build --lib --target wasm32-unknown-unknown --release

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

# Build for Lambda (ARM64)
pip3 install cargo-lambda  # one-time setup
cargo lambda build --release --arm64 --features lambda --bin lambda
```

## CLI Tool

The `otlp2pipeline` CLI manages Cloudflare infrastructure (R2, Pipelines). Install with:

```bash
cargo install --path .
```

### Commands

```bash
# Initialize project config (creates .otlp2pipeline.toml)
otlp2pipeline init --provider cf --env prod
otlp2pipeline init --provider cf --env prod --worker-url https://my-worker.workers.dev
otlp2pipeline init --provider azure --env prod --region westus

# After init, commands auto-route via config (no 'cf' prefix needed)
# Create environment (bucket, streams, sinks, pipelines)
otlp2pipeline create --r2-token $R2_TOKEN --output wrangler.toml
otlp2pipeline create --env staging  # override config

# Check status
otlp2pipeline status
otlp2pipeline status --env prod

# Dry run (show what would be created)
otlp2pipeline plan

# Tear down
otlp2pipeline destroy --force
otlp2pipeline destroy --env staging --force

# Query data with DuckDB
otlp2pipeline query

# Explicit provider (skip config): use 'cf' or 'cloudflare' subcommand
otlp2pipeline cf create --r2-token $R2_TOKEN --output wrangler.toml

# AWS deployment (full orchestration)
otlp2pipeline aws create --env prod --region us-east-1

# AWS deployment with local Lambda build
otlp2pipeline aws create --env prod --region us-east-1 --local

# AWS dry-run (show what would be created)
otlp2pipeline aws plan --env prod

# Check AWS status
otlp2pipeline aws status --env prod --region us-east-1

# AWS teardown
otlp2pipeline aws destroy --env prod --region us-east-1 --force
```

### Azure Prerequisites

Azure deployment requires the Azure CLI to be installed and authenticated:

```bash
# Install Azure CLI (if not already installed)
# macOS:
brew install azure-cli

# Linux:
curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash

# Windows:
# Download from https://aka.ms/installazurecliwindows

# Authenticate
az login

# Verify authentication and view subscription
az account show

# (Optional) Set a specific subscription if you have multiple
az account set --subscription <subscription-id-or-name>
```

For installation details, see: https://learn.microsoft.com/cli/azure/install-azure-cli

### Azure Commands

```bash
# Azure deployment (full orchestration)
otlp2pipeline azure create --env prod --region westus

# Azure dry-run (show what would be created)
otlp2pipeline azure plan --env prod

# Check Azure status
otlp2pipeline azure status --env prod

# Azure teardown
otlp2pipeline azure destroy --env prod --force

# List known services
otlp2pipeline services --url https://my-worker.workers.dev
otlp2pipeline services  # uses worker_url from config

# Stream live logs for a service
otlp2pipeline tail my-service logs --url https://my-worker.workers.dev
otlp2pipeline tail my-service logs  # uses worker_url from config

# Stream live traces
otlp2pipeline tail api-gateway traces
```

### Config File

The `init` command creates `.otlp2pipeline.toml` in the current directory:

```toml
provider = "cloudflare"
environment = "prod"
worker_url = "https://my-worker.workers.dev"  # optional
account_id = "abc123"                          # optional
```

URL resolution cascade: `--url` flag > `.otlp2pipeline.toml` > `wrangler.toml`

### Naming

Environment names are normalized - the `otlp2pipeline-` prefix is optional:
- `prod` and `otlp2pipeline-prod` both resolve to bucket `otlp2pipeline-prod`
- Naming logic lives in `src/cli/commands/naming.rs`

### Auth

The CLI resolves credentials in order:
1. `CF_API_TOKEN` environment variable
2. Wrangler OAuth token from `~/.wrangler/config/default.toml`

Account ID is auto-detected from the API, or set `CF_ACCOUNT_ID` explicitly.

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
        → Triple-write:
            → PipelineSender::send_all (required - forward to Cloudflare Pipeline)
            → AggregatorSender::send_to_aggregator (best-effort - RED metrics)
            → LiveTailSender::send_to_livetail (best-effort - WebSocket streaming)
```

### SignalHandler Trait

Each telemetry signal type (logs, traces) implements `SignalHandler` in `src/handler/mod.rs`:
- `Signal` enum in `src/signal.rs` defines all supported types
- Handler provides decode function and VRL program reference
- Generic `handle_signal<H>` processes any handler type

### OTLP Decoding + Transform

Core decode + VRL transform live in `otlp2records`. This crate only orchestrates:
- HTTP handling, gzip detection, and routing by signal
- Sending transformed JSON records to pipelines, aggregator, and livetail

### Schema Unification (`build.rs`)

Schema definitions come from `otlp2records` and are emitted to `schemas/*.schema.json` at build time for Cloudflare Pipeline configuration.

### Aggregator (`src/aggregator/`)

Durable Objects compute baseline RED metrics (Rate, Errors, Duration) per service:

- `stats.rs`: `LogAggregates` and `TraceAggregates` types for in-memory accumulation
- `durable_object.rs`: `AggregatorDO` with SQLite storage per {service}:{signal}
- `sender.rs`: Routes logs/traces to appropriate DO instances (metrics skip aggregator)

Each DO stores one row per minute with aggregated counts:
- Logs: `count`, `error_count` (severity >= 17)
- Traces: `count`, `error_count` (status_code == 2), `latency_sum_us`, `latency_min_us`, `latency_max_us`

Query endpoints:
- `GET /v1/services/:service/:signal/stats?from=X&to=Y` - stats for a single service
- `GET /v1/services/stats?signal=logs|traces&from=X&to=Y` - stats for all services (fan-out)

### Registry (`src/registry/`)

Singleton Durable Object tracking all services seen:

- `durable_object.rs`: `RegistryDO` with SQLite storage, 10,000 service limit
- `cache.rs`: Worker-local cache with 3-minute TTL to minimize DO calls
- `sender.rs`: `RegistrySender` trait for abstraction

Service validation: alphanumeric + hyphens + underscores + dots, max 128 chars.

Query via `GET /v1/services` (returns all services with signal availability).

### Livetail (`src/livetail/`)

Durable Objects for real-time WebSocket streaming of logs and traces:

- `durable_object.rs`: `LiveTailDO` with WebSocket hibernation per {service}:{signal}
- `cache.rs`: Worker-local cache with 10s TTL to track which DOs have clients
- `sender.rs`: `LiveTailSender` trait for best-effort broadcast

WebSocket endpoint: `GET /v1/tail/:service/:signal` upgrades to WebSocket.
Workers fan-out transformed records to LiveTailDOs, which broadcast to connected clients.
Hibernation ensures zero cost when no clients are connected.

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
