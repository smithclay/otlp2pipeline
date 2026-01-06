pub mod auth;
pub mod commands;
pub mod url;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "frostbit")]
#[command(about = "Manage frostbit infrastructure on Cloudflare")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new pipeline environment
    Create(CreateArgs),
    /// Destroy a pipeline environment
    Destroy(DestroyArgs),
    /// Show environment status
    Status(StatusArgs),
    /// Dry-run: show what would be created
    Plan(PlanArgs),
    /// Start a DuckDB query session
    Query(QueryArgs),
    /// List known services
    Services(ServicesArgs),
    /// Stream live telemetry
    Tail(TailArgs),
    /// Manage Iceberg catalog
    Catalog(CatalogArgs),
    /// Manage R2 bucket data
    Bucket(BucketArgs),
    /// Generate OpenTelemetry Collector config
    Connect(ConnectArgs),
}

#[derive(clap::Args)]
pub struct CatalogArgs {
    #[command(subcommand)]
    pub command: CatalogCommands,
}

#[derive(clap::Args)]
pub struct BucketArgs {
    #[command(subcommand)]
    pub command: BucketCommands,
}

#[derive(Subcommand)]
pub enum BucketCommands {
    /// Delete all objects in the bucket using AWS CLI
    Delete(BucketDeleteArgs),
}

#[derive(clap::Args)]
pub struct BucketDeleteArgs {
    /// Environment name (bucket will be frostbit-{name})
    pub name: String,

    /// Override bucket name (use exact name instead of frostbit-{name})
    #[arg(long)]
    pub bucket: Option<String>,

    /// AWS Access Key ID for R2 S3 API
    #[arg(long, env = "AWS_ACCESS_KEY_ID")]
    pub access_key_id: String,

    /// AWS Secret Access Key for R2 S3 API
    #[arg(long, env = "AWS_SECRET_ACCESS_KEY")]
    pub secret_access_key: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub force: bool,
}

#[derive(Subcommand)]
pub enum CatalogCommands {
    /// List table metadata including partition specs
    List(CatalogListArgs),
    /// Add service_name identity partition to all tables
    Partition(CatalogPartitionArgs),
}

#[derive(clap::Args)]
pub struct CatalogListArgs {
    /// R2 API token (create at dash.cloudflare.com > R2 > Manage R2 API Tokens)
    #[arg(long = "r2-token", env = "R2_API_TOKEN")]
    pub r2_token: String,

    /// Path to wrangler.toml config file
    #[arg(long, default_value = "wrangler.toml")]
    pub config: String,
}

#[derive(clap::Args)]
pub struct CatalogPartitionArgs {
    /// R2 API token (create at dash.cloudflare.com > R2 > Manage R2 API Tokens)
    #[arg(long = "r2-token", env = "R2_API_TOKEN")]
    pub r2_token: String,

    /// Path to wrangler.toml config file
    #[arg(long, default_value = "wrangler.toml")]
    pub config: String,

    /// Show what would change without applying
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::Args)]
pub struct CreateArgs {
    /// Environment name
    pub name: String,

    /// R2 API token (create at dash.cloudflare.com > R2 > Manage R2 API Tokens)
    ///
    /// Required permissions: Admin Read & Write. This is separate from CF_API_TOKEN.
    #[arg(long = "r2-token", env = "R2_API_TOKEN")]
    pub r2_token: String,

    /// Path to write wrangler.toml (stdout if not specified)
    #[arg(long)]
    pub output: Option<String>,

    /// Enable logs signal
    #[arg(long, default_value = "true")]
    pub logs: bool,

    /// Enable traces signal
    #[arg(long, default_value = "true")]
    pub traces: bool,

    /// Enable metrics signals (gauge, sum)
    #[arg(long, default_value = "true")]
    pub metrics: bool,

    /// Enable RED metrics Durable Object
    #[arg(long, default_value = "true")]
    pub aggregator: bool,

    /// Enable WebSocket streaming Durable Object
    #[arg(long, default_value = "true")]
    pub livetail: bool,

    /// Aggregator retention in minutes
    #[arg(long, default_value = "60")]
    pub retention: u32,
}

#[derive(clap::Args)]
pub struct DestroyArgs {
    /// Environment name
    pub name: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub force: bool,
}

#[derive(clap::Args)]
pub struct StatusArgs {
    /// Environment name
    pub name: String,
}

#[derive(clap::Args)]
pub struct PlanArgs {
    /// Environment name
    pub name: String,
}

#[derive(clap::Args)]
pub struct QueryArgs {
    /// Environment name
    pub name: String,
}

#[derive(clap::Args)]
pub struct ServicesArgs {
    /// Worker URL (falls back to wrangler.toml)
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(clap::Args)]
pub struct TailArgs {
    /// Service name to tail
    pub service: String,

    /// Signal type (logs or traces)
    pub signal: String,

    /// Worker URL (falls back to wrangler.toml)
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(clap::Args)]
pub struct ConnectArgs {
    #[command(subcommand)]
    pub command: ConnectCommands,
}

#[derive(Subcommand)]
pub enum ConnectCommands {
    /// Generate OpenTelemetry Collector config (otel-collector-config.yaml)
    OtelCollector(ConnectOtelCollectorArgs),
    /// Generate shell exports for Claude Code integration
    ClaudeCode(ConnectClaudeCodeArgs),
}

#[derive(clap::Args)]
pub struct ConnectOtelCollectorArgs {
    /// Worker URL (falls back to wrangler.toml)
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(clap::Args)]
pub struct ConnectClaudeCodeArgs {
    /// Worker URL (falls back to wrangler.toml)
    #[arg(long)]
    pub url: Option<String>,

    /// Output format
    #[arg(long, default_value = "shell")]
    pub format: String,
}
