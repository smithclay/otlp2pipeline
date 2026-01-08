use anyhow::Result;

use super::naming::{bucket_name, normalize, pipeline_name, sink_name, stream_name};
use crate::cli::auth;
use crate::cli::CreateArgs;
use crate::cloudflare::{CloudflareClient, CorsAllowed, CorsRule, SchemaField};

/// Signal configuration
struct SignalConfig {
    name: &'static str,
    schema_file: &'static str,
    table: &'static str,
}

const SIGNALS: &[SignalConfig] = &[
    SignalConfig {
        name: "logs",
        schema_file: "schemas/logs.schema.json",
        table: "logs",
    },
    SignalConfig {
        name: "traces",
        schema_file: "schemas/spans.schema.json",
        table: "traces",
    },
    SignalConfig {
        name: "gauge",
        schema_file: "schemas/gauge.schema.json",
        table: "gauge",
    },
    SignalConfig {
        name: "sum",
        schema_file: "schemas/sum.schema.json",
        table: "sum",
    },
];

fn enabled_signals(args: &CreateArgs) -> Vec<&'static SignalConfig> {
    SIGNALS
        .iter()
        .filter(|s| match s.name {
            "logs" => args.logs,
            "traces" => args.traces,
            "gauge" | "sum" => args.metrics,
            _ => false,
        })
        .collect()
}

pub async fn execute_create(args: CreateArgs) -> Result<()> {
    eprintln!("==> Creating pipeline environment: {}", args.name);

    // Resolve auth
    eprintln!("==> Resolving credentials...");
    let creds = auth::resolve_credentials()?;
    let client = CloudflareClient::new(creds.token, creds.account_id).await?;
    eprintln!("    Account ID: {}", client.account_id());

    let bucket = bucket_name(&args.name);
    let signals = enabled_signals(&args);

    eprintln!("    Bucket: {}", bucket);
    eprintln!(
        "    Signals: {:?}",
        signals.iter().map(|s| s.name).collect::<Vec<_>>()
    );

    // Step 1: Create R2 bucket
    eprintln!("\n==> Creating R2 bucket: {}", bucket);
    match client.create_bucket(&bucket).await? {
        Some(_) => eprintln!("    Created"),
        None => eprintln!("    Already exists"),
    }

    // Step 1b: Set CORS for browser access (enables DuckDB Iceberg queries from browser)
    eprintln!("\n==> Setting bucket CORS policy...");
    client
        .set_bucket_cors(
            &bucket,
            vec![CorsRule {
                allowed: CorsAllowed {
                    origins: vec!["*".to_string()],
                    methods: vec!["GET".to_string(), "HEAD".to_string()],
                    headers: vec!["*".to_string()],
                },
                max_age_seconds: 86400,
            }],
        )
        .await?;
    eprintln!("    Set");

    // Step 2: Enable catalog
    eprintln!("\n==> Enabling R2 Data Catalog...");
    client.enable_catalog(&bucket).await?;
    eprintln!("    Enabled");

    // Step 3: Set service credential
    eprintln!("\n==> Setting service credential...");
    client
        .set_catalog_credential(&bucket, &args.r2_token)
        .await?;
    eprintln!("    Set");

    // Step 4: Configure maintenance
    eprintln!("\n==> Configuring catalog maintenance...");
    client.configure_catalog_maintenance(&bucket).await?;
    eprintln!("    Compaction: enabled");
    eprintln!("    Snapshot expiration: enabled (max_snapshot_age=1d)");

    // Step 5: Create streams
    eprintln!("\n==> Creating streams...");
    for signal in &signals {
        let name = stream_name(&args.name, signal.name);
        eprintln!("    Creating: {}", name);

        let schema = load_schema(signal.schema_file)?;
        match client.create_stream(&name, &schema).await? {
            Some(_) => eprintln!("      Created"),
            None => eprintln!("      Already exists"),
        }
    }

    // Step 6: Get stream endpoints
    eprintln!("\n==> Getting stream endpoints...");
    let streams = client.list_streams().await?;
    let mut endpoints: Vec<(&str, String)> = Vec::new();

    for signal in &signals {
        let name = stream_name(&args.name, signal.name);
        if let Some(stream) = streams.iter().find(|s| s.name == name) {
            if let Some(ref endpoint) = stream.endpoint {
                eprintln!("    {}: {}", signal.name, endpoint);
                endpoints.push((signal.name, endpoint.clone()));
            }
        }
    }

    // Step 7: Create sinks
    eprintln!("\n==> Creating sinks...");
    for signal in &signals {
        let name = sink_name(&args.name, signal.name);
        eprintln!("    Creating: {}", name);

        match client
            .create_sink(
                &name,
                &bucket,
                signal.table,
                &args.r2_token,
                args.rolling_interval,
            )
            .await?
        {
            Some(_) => eprintln!("      Created"),
            None => eprintln!("      Already exists"),
        }
    }

    // Step 8: Create pipelines
    eprintln!("\n==> Creating pipelines...");
    for signal in &signals {
        let name = pipeline_name(&args.name, signal.name);
        let stream = stream_name(&args.name, signal.name);
        let sink = sink_name(&args.name, signal.name);
        eprintln!("    Creating: {}", name);

        match client.create_pipeline(&name, &stream, &sink).await? {
            Some(_) => eprintln!("      Created"),
            None => eprintln!("      Already exists"),
        }
    }

    // Step 9: Generate wrangler.toml
    eprintln!("\n==> Generating wrangler.toml...");
    let wrangler_toml = generate_wrangler_toml(&args, &endpoints, client.account_id(), &bucket);

    match &args.output {
        Some(path) => {
            std::fs::write(path, &wrangler_toml)?;
            eprintln!("    Written to: {}", path);
        }
        None => {
            println!("{}", wrangler_toml);
        }
    }

    // Summary
    eprintln!("\n==========================================");
    eprintln!("ENVIRONMENT CREATED");
    eprintln!("==========================================\n");
    eprintln!("Next steps:");
    eprintln!("  1. Set pipeline auth token:");
    eprintln!("     npx wrangler secret put PIPELINE_AUTH_TOKEN");
    eprintln!();
    eprintln!("  2. Deploy:");
    eprintln!("     npx wrangler deploy");
    eprintln!();
    eprintln!("  3. IMPORTANT: After ingesting data, add partitioning for query performance:");
    eprintln!("     otlp2pipeline catalog partition --r2-token $R2_API_TOKEN");
    eprintln!();
    eprintln!("     This adds service_name partitioning to Iceberg tables. Without it,");
    eprintln!("     queries will scan all data instead of pruning by service.");

    Ok(())
}

fn load_schema(path: &str) -> Result<Vec<SchemaField>> {
    let content = std::fs::read_to_string(path)?;
    let schema: serde_json::Value = serde_json::from_str(&content)?;
    let fields: Vec<SchemaField> =
        serde_json::from_value(schema.get("fields").cloned().unwrap_or_default())?;
    Ok(fields)
}

fn generate_wrangler_toml(
    args: &CreateArgs,
    endpoints: &[(&str, String)],
    account_id: &str,
    bucket: &str,
) -> String {
    let mut toml = format!(
        r#"name = "otlp2pipeline-{}"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[vars]
"#,
        normalize(&args.name)
    );

    for (signal, endpoint) in endpoints {
        let var_name = format!("PIPELINE_{}", signal.to_uppercase());
        toml.push_str(&format!("{} = \"{}\"\n", var_name, endpoint));
    }

    // R2 Catalog configuration for Iceberg queries
    toml.push_str(&format!("R2_CATALOG_ACCOUNT_ID = \"{}\"\n", account_id));
    toml.push_str(&format!("R2_CATALOG_BUCKET = \"{}\"\n", bucket));

    toml.push_str(&format!(
        r#"AGGREGATOR_ENABLED = "{}"
AGGREGATOR_RETENTION_MINUTES = "{}"
LIVETAIL_ENABLED = "{}"

[observability]
enabled = true

[observability.logs]
invocation_logs = true
head_sampling_rate = 0.1

[observability.traces]
enabled = false
"#,
        args.aggregator, args.retention, args.livetail
    ));

    if args.aggregator || args.livetail {
        toml.push('\n');
    }

    if args.aggregator {
        toml.push_str(
            r#"[[durable_objects.bindings]]
name = "AGGREGATOR"
class_name = "AggregatorDO"

[[durable_objects.bindings]]
name = "REGISTRY"
class_name = "RegistryDO"

"#,
        );
    }

    if args.livetail {
        toml.push_str(
            r#"[[durable_objects.bindings]]
name = "LIVETAIL"
class_name = "LiveTailDO"

"#,
        );
    }

    // Migrations
    if args.aggregator {
        toml.push_str(
            r#"[[migrations]]
tag = "v1"
new_sqlite_classes = ["AggregatorDO"]

[[migrations]]
tag = "v2"
new_sqlite_classes = ["RegistryDO"]

"#,
        );
    }

    if args.livetail {
        toml.push_str(
            r#"[[migrations]]
tag = "v3"
new_classes = ["LiveTailDO"]
"#,
        );
    }

    toml
}
