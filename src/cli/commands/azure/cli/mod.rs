// src/cli/commands/azure/cli/mod.rs
mod az;
mod eventhub;
mod resource;
mod storage;
mod stream_analytics;

pub use az::{AccountCli, AzureCli};
pub use eventhub::EventHubCli;
pub use resource::ResourceCli;
pub use storage::StorageCli;
pub use stream_analytics::StreamAnalyticsCli;
