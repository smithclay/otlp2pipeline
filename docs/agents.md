# Agent Guide

Quick reference for AI agents working with this codebase.

## Endpoints

| Path | Format | Handler |
|------|--------|---------|
| `POST /v1/logs` | OTLP JSON/Protobuf | `LogsHandler` |
| `POST /v1/traces` | OTLP JSON/Protobuf | `TracesHandler` |
| `POST /v1/metrics` | OTLP JSON/Protobuf | `handle_metrics` |
| `POST /services/collector/event` | Splunk HEC JSON/NDJSON | `HecLogsHandler` |
| `GET /health` | - | Returns "ok" |

## Architecture

```
Request → parse_content_metadata → handle_signal<H>
    → decompress_if_gzipped
    → H::decode (format → VRL Values)
    → VrlTransformer::transform_batch (VRL program)
    → PipelineSender::send_all (forward to Cloudflare Pipeline)
```

## Adding a New Signal Type

1. **Decoder** (`src/decode/`): Create decode module that returns `Vec<vrl::value::Value>`
2. **VRL Script** (`vrl/*.vrl`): Create transformation script, must set `._table`
3. **Build** (`build.rs`): Add script to `scripts` array
4. **Runtime** (`src/transform/runtime.rs`): Add `*_PROGRAM` static
5. **Handler** (`src/handler.rs`): Implement `SignalHandler` trait
6. **Routes** (`src/lib.rs`): Add to both WASM and native entry points
7. **Tests**: Add E2E test in `tests/`

## SignalHandler Trait

```rust
pub trait SignalHandler {
    const SIGNAL: Signal;
    fn decode(body: Bytes, format: DecodeFormat) -> Result<Vec<Value>, DecodeError>;
    fn vrl_program() -> &'static vrl::compiler::Program;
}
```

## Key Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | Entry points (WASM + native) |
| `src/handler.rs` | Signal handlers, decompression |
| `src/decode/` | Format decoders (OTLP, HEC) |
| `src/transform/` | VRL runtime, custom functions |
| `src/pipeline/` | Pipeline client, retry logic |
| `vrl/*.vrl` | Transformation scripts |
| `build.rs` | Compiles VRL at build time |

## Testing

```bash
cargo test                    # All tests
cargo test --test e2e_logs    # Specific E2E test
cargo build --target wasm32-unknown-unknown --release  # WASM build
./scripts/check-size.sh       # Bundle size check
```

## Dual-Target Build

Uses `#[cfg(target_arch = "wasm32")]` for:
- WASM: `worker` crate, `worker::Fetch`
- Native: `axum`, `reqwest`

Both share the same decode/transform logic.
