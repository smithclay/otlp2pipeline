// src/cli/commands/azure/mod.rs
mod cli;
mod context;
mod create;
pub mod deploy; // public for create.rs to use deploy functions
mod destroy;
mod helpers;
mod plan;
mod status;

pub use create::execute_create;
pub use destroy::execute_destroy;
pub use plan::execute_plan;
pub use status::execute_status;
