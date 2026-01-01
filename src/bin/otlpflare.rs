use clap::Parser;
use otlpflare::cli::{commands, Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create(args) => commands::execute_create(args).await?,
        Commands::Destroy(args) => {
            println!("Would destroy environment: {}", args.name);
        }
        Commands::Status(args) => {
            println!("Would show status for: {}", args.name);
        }
        Commands::Plan(args) => {
            println!("Would show plan for: {}", args.name);
        }
        Commands::Query(args) => {
            println!("Would start query session for: {}", args.name);
        }
    }

    Ok(())
}
