// src/aggregator/mod.rs
//! Signal aggregator using Durable Objects for baseline RED metrics.

mod stats;

#[cfg(target_arch = "wasm32")]
mod durable_object;

pub use stats::{LogAggregates, TraceAggregates};

#[cfg(target_arch = "wasm32")]
pub use durable_object::AggregatorDO;

// Native placeholder for tests
#[cfg(not(target_arch = "wasm32"))]
pub struct AggregatorDO;
