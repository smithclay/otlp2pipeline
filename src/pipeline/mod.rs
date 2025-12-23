// src/pipeline/mod.rs
pub mod client;
pub mod retry;
pub mod sender;

pub use client::PipelineClient;
pub use sender::{PipelineSender, SendResult};
