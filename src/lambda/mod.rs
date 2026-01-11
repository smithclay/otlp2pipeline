//! AWS Lambda-specific modules.

pub mod firehose;

// Re-export RetryConfig for testing
pub use crate::pipeline::retry::RetryConfig;
