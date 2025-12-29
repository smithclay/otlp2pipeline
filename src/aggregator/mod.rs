// src/aggregator/mod.rs
//! Signal aggregator using Durable Objects for baseline RED metrics.

mod stats;

pub use stats::{LogAggregates, TraceAggregates};

// Native placeholder for tests
#[cfg(not(target_arch = "wasm32"))]
pub struct AggregatorDO;
