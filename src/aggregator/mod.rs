// src/aggregator/mod.rs
//! Signal aggregator using Durable Objects for baseline RED metrics.

mod stats;

#[cfg(target_arch = "wasm32")]
mod durable_object;

#[cfg(target_arch = "wasm32")]
mod sender;

#[cfg(not(target_arch = "wasm32"))]
mod sender;

pub use stats::{LogAggregates, TraceAggregates};

#[cfg(target_arch = "wasm32")]
pub use durable_object::AggregatorDO;

#[cfg(target_arch = "wasm32")]
pub use sender::{AggregatorSender, WasmAggregatorSender};

#[cfg(not(target_arch = "wasm32"))]
pub use sender::{AggregatorSender, NativeAggregatorSender};

// Native placeholder for tests
#[cfg(not(target_arch = "wasm32"))]
pub struct AggregatorDO;
