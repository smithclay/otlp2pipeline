// src/cli/commands/azure/cli/mod.rs
mod az;
mod eventhub;
mod resource;
// mod storage;           // TODO: uncomment in Task 3
// mod stream_analytics;  // TODO: uncomment in Task 3

pub use az::AzureCli;
pub use resource::ResourceCli;
// pub use storage::StorageCli;
// pub use stream_analytics::StreamAnalyticsCli;
