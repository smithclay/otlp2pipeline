use clap::Parser;
use otlpflare::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create(args) => {
            println!("Would create environment: {}", args.name);
            println!("  token: {}...", &args.token[..8.min(args.token.len())]);
            println!("  logs: {}", args.logs);
            println!("  traces: {}", args.traces);
            println!("  metrics: {}", args.metrics);
        }
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
