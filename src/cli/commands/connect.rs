use anyhow::Result;

use crate::cli::config::try_load_config;
use crate::cli::url::resolve_worker_url;
use crate::cli::{ConnectClaudeCodeArgs, ConnectCodexArgs, ConnectOtelCollectorArgs};

/// Get auth token from config if present
fn get_auth_token() -> Option<String> {
    try_load_config().and_then(|c| c.auth_token)
}

/// Generate OpenTelemetry Collector configuration
pub async fn execute_connect_otel_collector(args: ConnectOtelCollectorArgs) -> Result<()> {
    let url = resolve_worker_url(args.url.as_deref()).await?;
    let auth_token = get_auth_token();

    let config = generate_collector_config(&url, auth_token.as_deref());
    println!("{}", config);

    Ok(())
}

/// Generate Claude Code shell exports
pub async fn execute_connect_claude_code(args: ConnectClaudeCodeArgs) -> Result<()> {
    let url = resolve_worker_url(args.url.as_deref()).await?;
    let auth_token = get_auth_token();

    let output = generate_claude_code_config(&url, &args.format, auth_token.as_deref());
    println!("{}", output);

    Ok(())
}

/// Generate OpenAI Codex CLI configuration
pub async fn execute_connect_codex(args: ConnectCodexArgs) -> Result<()> {
    let url = resolve_worker_url(args.url.as_deref()).await?;
    let auth_token = get_auth_token();

    let config = generate_codex_config(&url, auth_token.as_deref());
    println!("{}", config);

    Ok(())
}

fn generate_collector_config(endpoint: &str, auth_token: Option<&str>) -> String {
    let headers = match auth_token {
        Some(token) => format!(
            r#"
    headers:
      Authorization: "Bearer {}""#,
            token
        ),
        None => String::new(),
    };

    format!(
        r#"# OpenTelemetry Collector configuration for otlp2pipeline
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
    compression: gzip{headers}

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
        endpoint = endpoint,
        headers = headers
    )
}

fn generate_claude_code_config(endpoint: &str, format: &str, auth_token: Option<&str>) -> String {
    match format {
        "json" => generate_claude_code_json(endpoint, auth_token),
        _ => generate_claude_code_shell(endpoint, auth_token),
    }
}

fn generate_claude_code_shell(endpoint: &str, auth_token: Option<&str>) -> String {
    let headers_line = match auth_token {
        Some(token) => format!(
            r#"export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer {}"
"#,
            token
        ),
        None => String::new(),
    };

    format!(
        r#"# Claude Code OpenTelemetry configuration for otlp2pipeline
# Add to your shell profile (~/.bashrc, ~/.zshrc) or run before starting claude:

export CLAUDE_CODE_ENABLE_TELEMETRY=1
export OTEL_METRICS_EXPORTER=otlp
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT={endpoint}
{headers_line}
# Optional: reduce export intervals for faster updates (in milliseconds)
# export OTEL_METRIC_EXPORT_INTERVAL=10000
# export OTEL_LOGS_EXPORT_INTERVAL=5000

# Optional: include user prompts in logs (disabled by default for privacy)
# export OTEL_LOG_USER_PROMPTS=1

# Optional: add team/department labels for cost attribution
# export OTEL_RESOURCE_ATTRIBUTES=department=engineering,team.id=platform
"#,
        endpoint = endpoint,
        headers_line = headers_line
    )
}

fn generate_claude_code_json(endpoint: &str, auth_token: Option<&str>) -> String {
    let headers_json = match auth_token {
        Some(token) => format!(
            r#",
    "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer {}""#,
            token
        ),
        None => String::new(),
    };

    format!(
        r#"# Claude Code managed-settings.json for otlp2pipeline
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
    "OTEL_EXPORTER_OTLP_ENDPOINT": "{endpoint}"{headers_json}
  }}
}}"#,
        endpoint = endpoint,
        headers_json = headers_json
    )
}

fn generate_codex_config(endpoint: &str, auth_token: Option<&str>) -> String {
    let headers_section = match auth_token {
        Some(token) => format!(
            r#"
[otel.exporter."otlp-http".headers]
"Authorization" = "Bearer {}""#,
            token
        ),
        None => r#"
# Optional: add auth header if AUTH_TOKEN is set on the endpoint
# [otel.exporter."otlp-http".headers]
# "Authorization" = "Bearer <your-token>""#
            .to_string(),
    };

    format!(
        r#"# OpenAI Codex CLI configuration for otlp2pipeline
# Add to your codex config file (~/.codex/config.toml)
# or run: codex config --edit

[otel]
# Enable OTLP HTTP exporter for logs
exporter = "otlp-http"

# Optional: also enable trace export
# trace_exporter = "otlp-http"

# Optional: log user prompts (disabled by default for privacy)
# log_user_prompt = true

# Optional: environment label for filtering
# environment = "dev"

[otel.exporter."otlp-http"]
endpoint = "{endpoint}/v1/logs"
protocol = "binary"
{headers_section}
"#,
        endpoint = endpoint,
        headers_section = headers_section
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_collector_config() {
        let config = generate_collector_config("https://my-worker.workers.dev", None);
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
    fn test_generate_collector_config_with_auth() {
        let config =
            generate_collector_config("https://my-worker.workers.dev", Some("test-token-123"));
        assert!(config.contains("headers:"));
        assert!(config.contains("Authorization: \"Bearer test-token-123\""));
    }

    #[test]
    fn test_generate_claude_code_shell() {
        let config = generate_claude_code_shell("https://my-worker.workers.dev", None);
        assert!(config.contains("CLAUDE_CODE_ENABLE_TELEMETRY=1"));
        assert!(config.contains("OTEL_METRICS_EXPORTER=otlp"));
        assert!(config.contains("OTEL_LOGS_EXPORTER=otlp"));
        assert!(config.contains("OTEL_EXPORTER_OTLP_ENDPOINT=https://my-worker.workers.dev"));
        assert!(config.contains("http/protobuf"));
    }

    #[test]
    fn test_generate_claude_code_shell_with_auth() {
        let config =
            generate_claude_code_shell("https://my-worker.workers.dev", Some("test-token-123"));
        assert!(
            config.contains("OTEL_EXPORTER_OTLP_HEADERS=\"Authorization=Bearer test-token-123\"")
        );
    }

    #[test]
    fn test_generate_claude_code_json() {
        let config = generate_claude_code_json("https://my-worker.workers.dev", None);
        assert!(config.contains("\"CLAUDE_CODE_ENABLE_TELEMETRY\": \"1\""));
        assert!(
            config.contains("\"OTEL_EXPORTER_OTLP_ENDPOINT\": \"https://my-worker.workers.dev\"")
        );
        // Should include file path instructions
        assert!(config.contains("managed-settings.json"));
        assert!(config.contains("/etc/claude-code/"));
    }

    #[test]
    fn test_generate_claude_code_json_with_auth() {
        let config =
            generate_claude_code_json("https://my-worker.workers.dev", Some("test-token-123"));
        assert!(config
            .contains("\"OTEL_EXPORTER_OTLP_HEADERS\": \"Authorization=Bearer test-token-123\""));
    }

    #[test]
    fn test_generate_codex_config() {
        let config = generate_codex_config("https://my-worker.workers.dev", None);
        assert!(config.contains("[otel]"));
        assert!(config.contains("exporter = \"otlp-http\""));
        assert!(config.contains("[otel.exporter.\"otlp-http\"]"));
        assert!(config.contains("endpoint = \"https://my-worker.workers.dev/v1/logs\""));
        assert!(config.contains("protocol = \"binary\""));
        // Should include config file path instructions
        assert!(config.contains("~/.codex/config.toml"));
    }

    #[test]
    fn test_generate_codex_config_with_auth() {
        let config = generate_codex_config("https://my-worker.workers.dev", Some("test-token-123"));
        assert!(config.contains("[otel.exporter.\"otlp-http\".headers]"));
        assert!(config.contains("\"Authorization\" = \"Bearer test-token-123\""));
    }
}
