use clap::Parser;
use frostbit::cli::{commands, Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create(args) => commands::execute_create(args).await?,
        Commands::Destroy(args) => commands::execute_destroy(args).await?,
        Commands::Status(args) => commands::execute_status(args).await?,
        Commands::Plan(args) => commands::execute_plan(args).await?,
        Commands::Query(args) => commands::execute_query(args).await?,
        Commands::Services(args) => commands::execute_services(args).await?,
        Commands::Tail(args) => commands::execute_tail(args).await?,
    }

    Ok(())
}
