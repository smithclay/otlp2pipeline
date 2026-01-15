// src/cli/commands/azure/cli/mod.rs
mod az;
mod eventhub;
mod functionapp;
mod resource;
mod storage;
mod stream_analytics;

pub use az::AzureCli;
pub use eventhub::EventHubCli;
#[allow(unused_imports)]
pub use functionapp::FunctionAppCli;
pub use resource::ResourceCli;
pub use storage::StorageCli;
pub use stream_analytics::{EventHubInputConfig, ParquetOutputConfig, StreamAnalyticsCli};
