use anyhow::Result;

use crate::cli::url::resolve_worker_url;
use crate::cli::{ConnectClaudeCodeArgs, ConnectOtelCollectorArgs};

/// Generate OpenTelemetry Collector configuration
pub async fn execute_connect_otel_collector(args: ConnectOtelCollectorArgs) -> Result<()> {
    let url = resolve_worker_url(args.url.as_deref()).await?;

    let config = generate_collector_config(&url);
    println!("{}", config);

    Ok(())
}

/// Generate Claude Code shell exports
pub async fn execute_connect_claude_code(args: ConnectClaudeCodeArgs) -> Result<()> {
    let url = resolve_worker_url(args.url.as_deref()).await?;

    let output = generate_claude_code_config(&url, &args.format);
    println!("{}", output);

    Ok(())
}

fn generate_collector_config(endpoint: &str) -> String {
    format!(
        r#"# OpenTelemetry Collector configuration for frostbit
# Save as otel-collector-config.yaml and run:
#   otelcol --config otel-collector-config.yaml

receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  batch:
    # Batch by resource (includes service.name) to reduce requests
    send_batch_size: 1000
    send_batch_max_size: 2000
    timeout: 5s

exporters:
  otlphttp:
    endpoint: {endpoint}
    compression: gzip

service:
  pipelines:
    logs:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlphttp]
    traces:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlphttp]
    metrics:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlphttp]
"#,
        endpoint = endpoint
    )
}

fn generate_claude_code_config(endpoint: &str, format: &str) -> String {
    match format {
        "json" => generate_claude_code_json(endpoint),
        _ => generate_claude_code_shell(endpoint),
    }
}

fn generate_claude_code_shell(endpoint: &str) -> String {
    format!(
        r#"# Claude Code OpenTelemetry configuration for frostbit
# Add to your shell profile (~/.bashrc, ~/.zshrc) or run before starting claude:

export CLAUDE_CODE_ENABLE_TELEMETRY=1
export OTEL_METRICS_EXPORTER=otlp
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT={endpoint}

# Optional: reduce export intervals for faster updates (in milliseconds)
# export OTEL_METRIC_EXPORT_INTERVAL=10000
# export OTEL_LOGS_EXPORT_INTERVAL=5000

# Optional: include user prompts in logs (disabled by default for privacy)
# export OTEL_LOG_USER_PROMPTS=1

# Optional: add team/department labels for cost attribution
# export OTEL_RESOURCE_ATTRIBUTES=department=engineering,team.id=platform
"#,
        endpoint = endpoint
    )
}

fn generate_claude_code_json(endpoint: &str) -> String {
    format!(
        r#"# Claude Code managed-settings.json for frostbit
# Save to one of these locations (requires admin privileges):
#   macOS:     /Library/Application Support/ClaudeCode/managed-settings.json
#   Linux/WSL: /etc/claude-code/managed-settings.json
#   Windows:   C:\Program Files\ClaudeCode\managed-settings.json

{{
  "env": {{
    "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
    "OTEL_METRICS_EXPORTER": "otlp",
    "OTEL_LOGS_EXPORTER": "otlp",
    "OTEL_EXPORTER_OTLP_PROTOCOL": "http/protobuf",
    "OTEL_EXPORTER_OTLP_ENDPOINT": "{endpoint}"
  }}
}}"#,
        endpoint = endpoint
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_collector_config() {
        let config = generate_collector_config("https://my-worker.workers.dev");
        assert!(config.contains("endpoint: https://my-worker.workers.dev"));
        assert!(config.contains("compression: gzip"));
        assert!(config.contains("processors: [batch]"));
        assert!(config.contains("send_batch_size: 1000"));
        // All three signal types should be present
        assert!(config.contains("logs:"));
        assert!(config.contains("traces:"));
        assert!(config.contains("metrics:"));
    }

    #[test]
    fn test_generate_claude_code_shell() {
        let config = generate_claude_code_shell("https://my-worker.workers.dev");
        assert!(config.contains("CLAUDE_CODE_ENABLE_TELEMETRY=1"));
        assert!(config.contains("OTEL_METRICS_EXPORTER=otlp"));
        assert!(config.contains("OTEL_LOGS_EXPORTER=otlp"));
        assert!(config.contains("OTEL_EXPORTER_OTLP_ENDPOINT=https://my-worker.workers.dev"));
        assert!(config.contains("http/protobuf"));
    }

    #[test]
    fn test_generate_claude_code_json() {
        let config = generate_claude_code_json("https://my-worker.workers.dev");
        assert!(config.contains("\"CLAUDE_CODE_ENABLE_TELEMETRY\": \"1\""));
        assert!(
            config.contains("\"OTEL_EXPORTER_OTLP_ENDPOINT\": \"https://my-worker.workers.dev\"")
        );
        // Should include file path instructions
        assert!(config.contains("managed-settings.json"));
        assert!(config.contains("/etc/claude-code/"));
    }
}
