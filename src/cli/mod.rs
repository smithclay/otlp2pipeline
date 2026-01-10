pub mod auth;
pub mod commands;
pub mod config;
pub mod url;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "otlp2pipeline")]
#[command(about = "Manage otlp2pipeline infrastructure on Cloudflare")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize project config (.otlp2pipeline.toml)
    Init(InitArgs),

    // Top-level commands (auto-route via config)
    /// Create pipeline environment (reads provider from .otlp2pipeline.toml)
    Create(CreateArgs),
    /// Destroy pipeline environment (reads provider from .otlp2pipeline.toml)
    Destroy(DestroyArgs),
    /// Show environment status (reads provider from .otlp2pipeline.toml)
    Status(StatusArgs),
    /// Dry-run: show what would be created (reads provider from .otlp2pipeline.toml)
    Plan(PlanArgs),
    /// Start a DuckDB query session (reads provider from .otlp2pipeline.toml)
    Query(QueryArgs),

    // Provider-specific subcommands (explicit)
    /// Cloudflare infrastructure commands (explicit provider)
    #[command(alias = "cf")]
    Cloudflare(CloudflareArgs),

    // Provider-agnostic commands
    /// List known services
    Services(ServicesArgs),
    /// Stream live telemetry
    Tail(TailArgs),
    /// Generate OpenTelemetry Collector config
    Connect(ConnectArgs),
}

#[derive(clap::Args)]
pub struct InitArgs {
    /// Cloud provider (cloudflare, cf)
    #[arg(long, short)]
    pub provider: String,

    /// Environment name
    #[arg(long, short)]
    pub env: String,

    /// Worker URL (optional, can be set later)
    #[arg(long)]
    pub worker_url: Option<String>,

    /// Overwrite existing config
    #[arg(long)]
    pub force: bool,
}

#[derive(clap::Args)]
pub struct CloudflareArgs {
    #[command(subcommand)]
    pub command: CloudflareCommands,
}

#[derive(Subcommand)]
pub enum CloudflareCommands {
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
    /// Manage Iceberg catalog
    Catalog(CatalogArgs),
    /// Manage R2 bucket data
    Bucket(BucketArgs),
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
    /// Environment name (bucket will be otlp2pipeline-{name})
    pub name: String,

    /// Override bucket name (use exact name instead of otlp2pipeline-{name})
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
    /// Environment name (overrides .otlp2pipeline.toml)
    #[arg(long)]
    pub env: Option<String>,

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

    /// Rolling policy interval in seconds (how often files are written to R2)
    #[arg(long, default_value = "300")]
    pub rolling_interval: u32,

    /// Build worker locally instead of downloading from GitHub releases
    #[arg(long)]
    pub use_local: bool,
}

#[derive(clap::Args)]
pub struct DestroyArgs {
    /// Environment name (overrides .otlp2pipeline.toml)
    #[arg(long)]
    pub env: Option<String>,

    /// Skip confirmation prompt
    #[arg(long)]
    pub force: bool,

    /// Also delete the worker script
    #[arg(long)]
    pub include_worker: bool,
}

#[derive(clap::Args)]
pub struct StatusArgs {
    /// Environment name (overrides .otlp2pipeline.toml)
    #[arg(long)]
    pub env: Option<String>,
}

#[derive(clap::Args)]
pub struct PlanArgs {
    /// Environment name (overrides .otlp2pipeline.toml)
    #[arg(long)]
    pub env: Option<String>,
}

#[derive(clap::Args)]
pub struct QueryArgs {
    /// Environment name (overrides .otlp2pipeline.toml)
    #[arg(long)]
    pub env: Option<String>,
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
    /// Generate TOML config for OpenAI Codex CLI
    Codex(ConnectCodexArgs),
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

#[derive(clap::Args)]
pub struct ConnectCodexArgs {
    /// Worker URL (falls back to wrangler.toml)
    #[arg(long)]
    pub url: Option<String>,
}
