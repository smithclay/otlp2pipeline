// src/registry/mod.rs
//! Service registry using Durable Objects for tracking known services.

pub mod cache;

#[cfg(target_arch = "wasm32")]
mod durable_object;

#[cfg(target_arch = "wasm32")]
pub use durable_object::RegistryDO;

// Native placeholder for tests
#[cfg(not(target_arch = "wasm32"))]
pub struct RegistryDO;
