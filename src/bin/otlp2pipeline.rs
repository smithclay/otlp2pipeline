use anyhow::{bail, Context};
use clap::Parser;
use otlp2pipeline::cli::{
    commands, config, AwsCommands, BucketCommands, CatalogCommands, Cli, CloudflareCommands,
    Commands, ConnectCommands, CreateArgs, DestroyArgs, PlanArgs, StatusArgs,
};

/// Load config or error with helpful message
fn require_config() -> anyhow::Result<config::Config> {
    config::Config::load().with_context(|| {
        format!(
            "No {} found. Run 'otlp2pipeline init' first, or use 'otlp2pipeline cf <command>' for explicit provider.",
            config::CONFIG_FILENAME
        )
    })
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
                region: args.region,
                force: args.force,
            };
            commands::execute_init(init_args)?
        }

        // Top-level commands: auto-route via config
        Commands::Create(args) => route_create(args).await?,
        Commands::Destroy(args) => route_destroy(args).await?,
        Commands::Status(args) => route_status(args).await?,
        Commands::Plan(args) => route_plan(args).await?,
        Commands::Query(args) => {
            let cfg = require_config()?;
            match cfg.provider.as_str() {
                "cloudflare" => commands::execute_query(args).await?,
                "aws" => commands::aws::execute_query(args)?,
                other => bail!("Provider '{}' not supported", other),
            }
        }

        // Explicit Cloudflare provider subcommand
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

        // Explicit AWS provider subcommand (sync functions - no await needed)
        Commands::Aws(aws_args) => match aws_args.command {
            AwsCommands::Create(args) => commands::aws::execute_create(args)?,
            AwsCommands::Status(args) => commands::aws::execute_status(args)?,
            AwsCommands::Plan(args) => commands::aws::execute_plan(args)?,
            AwsCommands::Destroy(args) => commands::aws::execute_destroy(args)?,
            AwsCommands::Query(args) => commands::aws::execute_query(args)?,
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

/// Route create command based on provider config
async fn route_create(args: CreateArgs) -> anyhow::Result<()> {
    let cfg = require_config()?;
    match cfg.provider.as_str() {
        "cloudflare" => commands::execute_create(args).await,
        "aws" => commands::aws::execute_create(args),
        other => bail!("Provider '{}' not supported", other),
    }
}

/// Route destroy command based on provider config
async fn route_destroy(args: DestroyArgs) -> anyhow::Result<()> {
    let cfg = require_config()?;
    match cfg.provider.as_str() {
        "cloudflare" => commands::execute_destroy(args).await,
        "aws" => commands::aws::execute_destroy(args),
        other => bail!("Provider '{}' not supported", other),
    }
}

/// Route status command based on provider config
async fn route_status(args: StatusArgs) -> anyhow::Result<()> {
    let cfg = require_config()?;
    match cfg.provider.as_str() {
        "cloudflare" => commands::execute_status(args).await,
        "aws" => commands::aws::execute_status(args),
        other => bail!("Provider '{}' not supported", other),
    }
}

/// Route plan command based on provider config
async fn route_plan(args: PlanArgs) -> anyhow::Result<()> {
    let cfg = require_config()?;
    match cfg.provider.as_str() {
        "cloudflare" => commands::execute_plan(args).await,
        "aws" => commands::aws::execute_plan(args),
        other => bail!("Provider '{}' not supported", other),
    }
}
