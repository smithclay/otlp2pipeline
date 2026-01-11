# otlp2records Implementation Plan

A minimal, WASM-friendly Rust crate that transforms OTLP telemetry into Arrow RecordBatches with VRL transformation.

## Vision

```
OTLP bytes (protobuf/JSON)
       ↓
   Decode (prost/serde)
       ↓
   VRL Transform
       ↓
   Arrow RecordBatch
       ↓
   Output: Arrow IPC / Parquet / JSON
```

## Core Principles

| Principle | Implementation |
|-----------|----------------|
| **No I/O** | Core never touches network/filesystem |
| **No async** | Pure synchronous transforms |
| **No compression** | Caller handles gzip |
| **WASM-first** | All deps must compile to wasm32 |
| **Arrow-native** | RecordBatch is the canonical output |

## Lineage

Merges core functionality from:
- **otlp2pipeline**: VRL transformation, custom functions, schema-from-VRL approach
- **otlp2parquet-core**: Arrow RecordBatch construction, WASM patterns

---

## Phase 1: Scaffold & Foundation

### 1.1 Create repo and crate structure

```
otlp2records/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   └── error.rs
├── vrl/
│   └── .gitkeep
└── build.rs
```

### 1.2 Configure Cargo.toml

```toml
[package]
name = "otlp2records"
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
description = "Transform OTLP telemetry to Arrow RecordBatches"
keywords = ["opentelemetry", "otlp", "arrow", "observability"]
categories = ["data-structures", "encoding"]

[dependencies]
# Decoding
opentelemetry-proto = { version = "0.31", default-features = false }
prost = "0.14"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Arrow
arrow = { version = "54", default-features = false, features = ["ffi"] }

# VRL (no stdlib - avoids zstd C dependency)
vrl = { version = "0.29", default-features = false, features = ["compiler", "value"] }

# Utils
thiserror = "2"
const-hex = "1"
once_cell = "1"
chrono-tz = "0.10"

[dependencies.parquet]
version = "54"
optional = true
default-features = false
features = ["arrow"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["js"] }
wasm-bindgen = { version = "0.2", optional = true }

[features]
default = []
parquet = ["dep:parquet"]
wasm = ["getrandom/js", "dep:wasm-bindgen"]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

### 1.3 Set up build.rs

- Parse `@schema` annotations from VRL files
- Generate Arrow schema definitions
- Embed VRL source as compile-time constants in `$OUT_DIR/compiled_vrl.rs`

Source: Copy schema parser from `otlp2pipeline/build.rs`

---

## Phase 2: Decode Layer

### 2.1 Port OTLP decoders

| File | Source | Purpose |
|------|--------|---------|
| `src/decode/mod.rs` | new | InputFormat enum, public API |
| `src/decode/logs.rs` | otlp2pipeline | Logs protobuf + JSON |
| `src/decode/traces.rs` | otlp2pipeline | Traces protobuf + JSON |
| `src/decode/metrics.rs` | otlp2pipeline | Metrics protobuf + JSON |
| `src/decode/common.rs` | otlp2pipeline | Hex encoding, timestamps |

### 2.2 Simplifications from otlp2pipeline

Remove:
- HTTP Content-Type parsing (caller provides format)
- Gzip decompression (caller's job)
- SignalHandler trait (not needed)

Keep:
- Protobuf decoding via `opentelemetry-proto`
- JSON decoding via serde
- Output as `Vec<vrl::Value>`

### 2.3 Public decode API

```rust
pub enum InputFormat {
    Protobuf,
    Json,
}

pub fn decode_logs(bytes: &[u8], format: InputFormat) -> Result<Vec<Value>, DecodeError>;
pub fn decode_traces(bytes: &[u8], format: InputFormat) -> Result<Vec<Value>, DecodeError>;
pub fn decode_metrics(bytes: &[u8], format: InputFormat) -> Result<Vec<Value>, DecodeError>;
```

---

## Phase 3: Transform Layer

### 3.1 Port VRL runtime

| File | Source | Purpose |
|------|--------|---------|
| `src/transform/mod.rs` | otlp2pipeline | VrlTransformer |
| `src/transform/runtime.rs` | otlp2pipeline | Program compilation |
| `src/transform/functions/core.rs` | otlp2pipeline | stdlib replacements |
| `src/transform/functions/helpers.rs` | otlp2pipeline | OTLP-specific helpers |

### 3.2 Custom VRL functions (WASM-safe)

**Core (stdlib replacements):**
- `to_int` - Float/String → Integer
- `to_string` - Any → String
- `encode_json` - Object/Array → JSON string
- `get` - Object field access with fallback
- `is_empty` - Check if container empty
- `is_object` / `is_array` - Type checks
- `floor` - Math floor

**Helpers (OTLP-specific):**
- `string_or_null` - String or null if empty
- `nanos_to_millis` - Timestamp conversion
- `json_or_null` - JSON encode or null if empty
- `int_or_default` - Int with default
- `get_attr` - Attribute extraction

### 3.3 Port VRL programs

| File | Fields | Purpose |
|------|--------|---------|
| `vrl/otlp_logs.vrl` | 15 | Flatten log records |
| `vrl/otlp_traces.vrl` | 24 | Flatten span records |
| `vrl/otlp_gauge.vrl` | 20 | Flatten gauge metrics |
| `vrl/otlp_sum.vrl` | 20 | Flatten sum metrics |

### 3.4 Simplifications

Remove:
- `_table` field assignment (not needed in core)
- Batch grouping by table

Keep:
- All field transformations
- Schema annotations

---

## Phase 4: Arrow Layer

### 4.1 Schema generation

`src/arrow/schema.rs`

Map VRL schema types to Arrow DataTypes:

| VRL Type | Arrow DataType |
|----------|----------------|
| `timestamp` | `TimestampMillisecond(None)` |
| `int64` | `Int64` |
| `int32` | `Int32` |
| `float64` | `Float64` |
| `bool` | `Boolean` |
| `string` | `Utf8` |
| `json` | `Utf8` (JSON-encoded) |

### 4.2 RecordBatch builder

`src/arrow/builder.rs`

```rust
pub fn values_to_arrow(
    values: &[Value],
    schema: &Schema,
) -> Result<RecordBatch, ArrowError>;
```

Implementation:
1. Pre-allocate builders based on `values.len()`
2. Iterate values, extract fields by name
3. Coerce VRL types to Arrow types
4. Handle nulls (VRL `Null` → Arrow null)
5. Build arrays and construct RecordBatch

### 4.3 Public schema accessors

```rust
pub fn logs_schema() -> Schema;
pub fn traces_schema() -> Schema;
pub fn gauge_schema() -> Schema;
pub fn sum_schema() -> Schema;
```

---

## Phase 5: Output Serialization

### 5.1 JSON output

`src/output/json.rs`

```rust
pub fn to_json(batch: &RecordBatch) -> Result<Vec<u8>, Error>;
```

- NDJSON format (one JSON object per row)
- Use `arrow-json` or manual serialization

### 5.2 Arrow IPC output

`src/output/ipc.rs`

```rust
pub fn to_ipc(batch: &RecordBatch) -> Result<Vec<u8>, Error>;
```

- Streaming IPC format
- Useful for cross-language interop (Python, JS)

### 5.3 Parquet output (optional)

`src/output/parquet.rs` (behind `parquet` feature)

```rust
#[cfg(feature = "parquet")]
pub fn to_parquet(batch: &RecordBatch) -> Result<Vec<u8>, Error>;
```

- Single RecordBatch → Parquet bytes
- Use `parquet::arrow::ArrowWriter`

---

## Phase 6: Public API

### 6.1 High-level API

```rust
// src/lib.rs

/// Transform OTLP logs to Arrow RecordBatch
pub fn transform_logs(
    bytes: &[u8],
    format: InputFormat,
) -> Result<RecordBatch, Error>;

/// Transform OTLP traces to Arrow RecordBatch
pub fn transform_traces(
    bytes: &[u8],
    format: InputFormat,
) -> Result<RecordBatch, Error>;

/// Transform OTLP metrics to Arrow RecordBatches
/// Returns separate batches for gauge and sum metrics
pub fn transform_metrics(
    bytes: &[u8],
    format: InputFormat,
) -> Result<MetricBatches, Error>;

pub struct MetricBatches {
    pub gauge: Option<RecordBatch>,
    pub sum: Option<RecordBatch>,
}
```

### 6.2 Lower-level API

```rust
// For users who want control over individual steps

pub fn decode_logs(bytes: &[u8], format: InputFormat) -> Result<Vec<Value>, DecodeError>;
pub fn decode_traces(bytes: &[u8], format: InputFormat) -> Result<Vec<Value>, DecodeError>;
pub fn decode_metrics(bytes: &[u8], format: InputFormat) -> Result<Vec<Value>, DecodeError>;

pub fn apply_log_transform(values: Vec<Value>) -> Result<Vec<Value>, TransformError>;
pub fn apply_trace_transform(values: Vec<Value>) -> Result<Vec<Value>, TransformError>;
pub fn apply_metric_transform(values: Vec<Value>) -> Result<(Vec<Value>, Vec<Value>), TransformError>;

pub fn values_to_arrow(values: &[Value], schema: &Schema) -> Result<RecordBatch, ArrowError>;
```

### 6.3 Output helpers

```rust
pub fn to_json(batch: &RecordBatch) -> Result<Vec<u8>, Error>;
pub fn to_ipc(batch: &RecordBatch) -> Result<Vec<u8>, Error>;

#[cfg(feature = "parquet")]
pub fn to_parquet(batch: &RecordBatch) -> Result<Vec<u8>, Error>;
```

### 6.4 WASM bindings

`src/wasm.rs` (behind `wasm` feature)

```rust
#[wasm_bindgen]
pub fn transform_logs_wasm(bytes: &[u8], format: &str) -> Result<Vec<u8>, JsError>;

#[wasm_bindgen]
pub fn transform_traces_wasm(bytes: &[u8], format: &str) -> Result<Vec<u8>, JsError>;
```

Returns Arrow IPC bytes for use with DuckDB-WASM or arrow-js.

---

## Phase 7: Testing

### 7.1 Unit tests

| Area | Tests |
|------|-------|
| Decode | Protobuf fixtures, JSON fixtures, malformed input |
| Transform | VRL output validation, null handling |
| Arrow | Type coercion, schema conformance, null handling |
| Output | JSON round-trip, IPC validity |

### 7.2 Integration tests

```rust
#[test]
fn test_full_pipeline_logs() {
    let otlp_bytes = include_bytes!("fixtures/logs.pb");
    let batch = transform_logs(otlp_bytes, InputFormat::Protobuf).unwrap();
    assert_eq!(batch.num_rows(), 10);

    let json = to_json(&batch).unwrap();
    // Validate JSON structure
}
```

### 7.3 WASM tests

- Compile to `wasm32-unknown-unknown` in CI
- Size budget check (target: < 2MB)
- Optional: wasm-pack test in browser

### 7.4 Test fixtures

Port from otlp2pipeline:
- `tests/fixtures/logs.pb`
- `tests/fixtures/logs.json`
- `tests/fixtures/traces.pb`
- `tests/fixtures/traces.json`
- `tests/fixtures/metrics.pb`
- `tests/fixtures/metrics.json`

---

## Phase 8: Documentation & Release

### 8.1 Documentation

- `README.md` with usage examples
- API docs with `#[doc]` comments
- Architecture diagram (Mermaid)

### 8.2 CI/CD

`.github/workflows/ci.yml`:
- `cargo test`
- `cargo clippy`
- `cargo fmt --check`
- `cargo build --target wasm32-unknown-unknown`
- WASM size check

### 8.3 Release

- Publish to crates.io
- Tag releases with changelog

---

## Migration Path for otlp2pipeline

Once `otlp2records` is stable:

1. Add dependency:
   ```toml
   otlp2records = "0.1"
   ```

2. Replace decode layer:
   ```rust
   // Before
   use crate::decode::otlp::logs::decode_logs;

   // After
   use otlp2records::{decode_logs, InputFormat};
   ```

3. Replace transform layer:
   ```rust
   // Before
   use crate::transform::VrlTransformer;

   // After
   use otlp2records::apply_log_transform;
   ```

4. Convert Arrow → JSON for Pipeline API:
   ```rust
   let batch = otlp2records::transform_logs(&bytes, format)?;
   let json = otlp2records::to_json(&batch)?;
   pipeline.send(json).await?;
   ```

5. Keep I/O layer in otlp2pipeline:
   - Pipeline sender
   - Aggregator sender
   - LiveTail sender
   - HTTP routing

---

## Estimated Scope

| Directory | Files | Lines (est.) |
|-----------|-------|--------------|
| `src/decode/` | 5 | ~800 |
| `src/transform/` | 4 | ~400 |
| `src/arrow/` | 3 | ~400 |
| `src/output/` | 3 | ~200 |
| `src/` (root) | 3 | ~200 |
| `vrl/` | 4 | ~200 |
| `build.rs` | 1 | ~150 |
| tests | 4 | ~300 |
| **Total** | ~27 | ~2650 |

---

## Dependencies Summary

| Crate | Purpose | WASM Safe |
|-------|---------|-----------|
| `opentelemetry-proto` | OTLP protobuf types | Yes |
| `prost` | Protobuf decoding | Yes |
| `serde` / `serde_json` | JSON decoding | Yes |
| `arrow` | RecordBatch construction | Yes |
| `vrl` | Transformation runtime | Yes (no stdlib) |
| `thiserror` | Error types | Yes |
| `const-hex` | Hex encoding | Yes |
| `once_cell` | Lazy statics | Yes |
| `chrono-tz` | Timezone handling | Yes |
| `parquet` (optional) | Parquet output | Yes |
