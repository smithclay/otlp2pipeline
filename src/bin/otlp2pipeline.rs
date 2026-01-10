use anyhow::{bail, Context};
use clap::Parser;
use otlp2pipeline::cli::{
    commands, config, AwsCommands, BucketCommands, CatalogCommands, Cli, CloudflareCommands,
    Commands, ConnectCommands,
};
use std::future::Future;

/// Load config or error with helpful message
fn require_config() -> anyhow::Result<config::Config> {
    config::Config::load().with_context(|| {
        format!(
            "No {} found. Run 'otlp2pipeline init' first, or use 'otlp2pipeline cf <command>' for explicit provider.",
            config::CONFIG_FILENAME
        )
    })
}

/// Route a command through config-based provider dispatch
/// Note: Top-level commands use Cloudflare-specific args. AWS users should use `aws` subcommand.
async fn route_via_config<F, Fut>(cloudflare_handler: F) -> anyhow::Result<()>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let cfg = require_config()?;
    match cfg.provider.as_str() {
        "cloudflare" => cloudflare_handler().await,
        "aws" => bail!(
            "AWS provider detected in config. Use explicit commands instead:\n  \
            otlp2pipeline aws create\n  \
            otlp2pipeline aws status\n  \
            otlp2pipeline aws plan\n  \
            otlp2pipeline aws destroy"
        ),
        other => bail!("Provider '{}' not supported", other),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => {
            let init_args = commands::InitArgs {
                provider: args.provider,
                env: args.env,
                worker_url: args.worker_url,
                force: args.force,
            };
            commands::execute_init(init_args)?
        }

        // Top-level commands: auto-route via config
        Commands::Create(args) => route_via_config(|| commands::execute_create(args)).await?,
        Commands::Destroy(args) => route_via_config(|| commands::execute_destroy(args)).await?,
        Commands::Status(args) => route_via_config(|| commands::execute_status(args)).await?,
        Commands::Plan(args) => route_via_config(|| commands::execute_plan(args)).await?,
        Commands::Query(args) => route_via_config(|| commands::execute_query(args)).await?,

        // Explicit provider subcommand
        Commands::Cloudflare(cf_args) => match cf_args.command {
            CloudflareCommands::Create(args) => commands::execute_create(args).await?,
            CloudflareCommands::Destroy(args) => commands::execute_destroy(args).await?,
            CloudflareCommands::Status(args) => commands::execute_status(args).await?,
            CloudflareCommands::Plan(args) => commands::execute_plan(args).await?,
            CloudflareCommands::Query(args) => commands::execute_query(args).await?,
            CloudflareCommands::Catalog(args) => match args.command {
                CatalogCommands::List(list_args) => {
                    commands::execute_catalog_list(list_args).await?
                }
                CatalogCommands::Partition(partition_args) => {
                    commands::execute_catalog_partition(partition_args).await?
                }
            },
            CloudflareCommands::Bucket(args) => match args.command {
                BucketCommands::Delete(delete_args) => {
                    commands::execute_bucket_delete(delete_args).await?
                }
            },
        },
        // Explicit AWS provider subcommand
        Commands::Aws(aws_args) => match aws_args.command {
            AwsCommands::Create(args) => commands::aws::execute_create(args).await?,
            AwsCommands::Status(args) => commands::aws::execute_status(args).await?,
            AwsCommands::Plan(args) => commands::aws::execute_plan(args).await?,
            AwsCommands::Destroy(args) => commands::aws::execute_destroy(args).await?,
        },

        Commands::Services(args) => commands::execute_services(args).await?,
        Commands::Tail(args) => commands::execute_tail(args).await?,
        Commands::Connect(args) => match args.command {
            ConnectCommands::OtelCollector(otel_args) => {
                commands::execute_connect_otel_collector(otel_args).await?
            }
            ConnectCommands::ClaudeCode(claude_args) => {
                commands::execute_connect_claude_code(claude_args).await?
            }
            ConnectCommands::Codex(codex_args) => {
                commands::execute_connect_codex(codex_args).await?
            }
        },
    }

    Ok(())
}
