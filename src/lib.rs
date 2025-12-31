// src/lib.rs
use decode::DecodeFormat;

// Re-export for native module
#[cfg(not(target_arch = "wasm32"))]
pub use bytes::Bytes;

pub mod aggregator;
mod convert;
mod decode;
mod handler;
mod pipeline;
pub mod registry;
mod schema;
mod signal;
mod transform;

pub use signal::Signal;

// Re-export tracing for use in other modules
pub use tracing;

// Re-export for tests
pub use handler::{
    handle_signal, HandleError, HandleResponse, HecLogsHandler, LogsHandler, MetricsHandler,
    SignalHandler, TracesHandler,
};
pub use pipeline::{PipelineSender, SendResult};

fn parse_content_metadata(mut header: impl FnMut(&str) -> Option<String>) -> (bool, DecodeFormat) {
    let is_gzipped = header("content-encoding")
        .map(|v| v.eq_ignore_ascii_case("gzip"))
        .unwrap_or(false);
    let decode_format = DecodeFormat::from_content_type(header("content-type").as_deref());
    (is_gzipped, decode_format)
}

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::build_router;
