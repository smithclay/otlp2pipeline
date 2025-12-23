// src/transform/mod.rs
pub mod functions;
pub mod runtime;

pub use runtime::{VrlError, VrlTransformer};

// Only export init_programs for WASM target (used in worker startup)
#[cfg(target_arch = "wasm32")]
pub use runtime::init_programs;
