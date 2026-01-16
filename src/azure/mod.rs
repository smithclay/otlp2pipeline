//! Azure-specific modules.

pub mod eventhub;

// Re-export for convenience
pub use eventhub::{EventHubConfig, EventHubSender};
