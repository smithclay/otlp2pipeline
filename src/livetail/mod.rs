//! LiveTail: WebSocket streaming of logs and traces to browsers.

mod cache;
mod sender;

#[cfg(target_arch = "wasm32")]
mod durable_object;

pub use sender::{LiveTailSendResult, LiveTailSender};

#[cfg(target_arch = "wasm32")]
pub use durable_object::LiveTailDO;

#[cfg(target_arch = "wasm32")]
pub use sender::WasmLiveTailSender;

#[cfg(not(target_arch = "wasm32"))]
pub use sender::NativeLiveTailSender;

// Native placeholder for tests
#[cfg(not(target_arch = "wasm32"))]
pub struct LiveTailDO;
