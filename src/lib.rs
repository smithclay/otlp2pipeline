// src/lib.rs

// Re-export for native module and lambda binary
#[cfg(not(target_arch = "wasm32"))]
pub use bytes::Bytes;

// Re-export InputFormat for external use
pub use otlp2records::decode::InputFormat;

pub mod aggregator;
mod handler;
pub mod livetail;
mod pipeline;
pub mod registry;
mod schema;
mod signal;

pub use signal::Signal;

// Re-export tracing for use in other modules
pub use tracing;

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;

#[cfg(not(target_arch = "wasm32"))]
pub mod cloudflare;

#[cfg(feature = "lambda")]
pub mod lambda;

// Re-export for tests
pub use handler::{
    handle_signal, HandleError, HandleResponse, LogsHandler, MetricsHandler, SignalHandler,
    TracesHandler,
};
pub use pipeline::{PipelineSender, SendResult};

fn parse_content_metadata(mut header: impl FnMut(&str) -> Option<String>) -> (bool, InputFormat) {
    let is_gzipped = header("content-encoding")
        .map(|v| v.eq_ignore_ascii_case("gzip"))
        .unwrap_or(false);
    let decode_format = InputFormat::from_content_type(header("content-type").as_deref());
    (is_gzipped, decode_format)
}

#[cfg(target_arch = "wasm32")]
mod stats;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::build_router;
