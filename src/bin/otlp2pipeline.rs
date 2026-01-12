use anyhow::{bail, Context};
use clap::Parser;
use otlp2pipeline::cli::{
    commands, config, AwsCatalogCommands, AwsCommands, BucketCommands, CatalogCommands, Cli,
    CloudflareCommands, Commands, ConnectCommands,
};

/// Resolved provider from config
enum Provider {
    Cloudflare,
    Aws,
}

/// Load config and resolve provider
fn require_provider() -> anyhow::Result<Provider> {
    let cfg = config::Config::load().with_context(|| {
        format!(
            "No {} found. Run 'otlp2pipeline init' first, or use 'otlp2pipeline cf <command>' for explicit provider.",
            config::CONFIG_FILENAME
        )
    })?;
    match cfg.provider.as_str() {
        "cloudflare" => Ok(Provider::Cloudflare),
        "aws" => Ok(Provider::Aws),
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
                region: args.region,
                force: args.force,
            };
            commands::execute_init(init_args)?
        }

        // Top-level commands: auto-route via config
        Commands::Create(args) => match require_provider()? {
            Provider::Cloudflare => commands::execute_create(args).await?,
            Provider::Aws => commands::aws::execute_create(args)?,
        },
        Commands::Destroy(args) => match require_provider()? {
            Provider::Cloudflare => commands::execute_destroy(args).await?,
            Provider::Aws => commands::aws::execute_destroy(args)?,
        },
        Commands::Status(args) => match require_provider()? {
            Provider::Cloudflare => commands::execute_status(args).await?,
            Provider::Aws => commands::aws::execute_status(args)?,
        },
        Commands::Plan(args) => match require_provider()? {
            Provider::Cloudflare => commands::execute_plan(args).await?,
            Provider::Aws => commands::aws::execute_plan(args)?,
        },
        Commands::Query(args) => match require_provider()? {
            Provider::Cloudflare => commands::execute_query(args).await?,
            Provider::Aws => commands::aws::execute_query(args)?,
        },

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
            AwsCommands::Catalog(args) => match args.command {
                AwsCatalogCommands::List(list_args) => {
                    commands::aws::execute_catalog_list(list_args)?
                }
            },
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
