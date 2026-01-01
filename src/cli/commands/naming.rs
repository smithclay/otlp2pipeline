/// Normalize environment name by stripping otlpflare prefix if present
pub fn normalize(name: &str) -> &str {
    name.strip_prefix("otlpflare-")
        .or_else(|| name.strip_prefix("otlpflare_"))
        .unwrap_or(name)
}

pub fn bucket_name(env: &str) -> String {
    format!("otlpflare-{}", normalize(env).replace('_', "-"))
}

pub fn stream_name(env: &str, signal: &str) -> String {
    format!("otlpflare_{}_{}", normalize(env).replace('-', "_"), signal)
}

pub fn sink_name(env: &str, signal: &str) -> String {
    format!(
        "otlpflare_{}_{}_sink",
        normalize(env).replace('-', "_"),
        signal
    )
}

pub fn pipeline_name(env: &str, signal: &str) -> String {
    format!("otlpflare_{}_{}", normalize(env).replace('-', "_"), signal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_strips_dash_prefix() {
        assert_eq!(normalize("otlpflare-test05"), "test05");
    }

    #[test]
    fn test_normalize_strips_underscore_prefix() {
        assert_eq!(normalize("otlpflare_test05"), "test05");
    }

    #[test]
    fn test_normalize_no_prefix() {
        assert_eq!(normalize("test05"), "test05");
    }

    #[test]
    fn test_bucket_name_with_prefix() {
        assert_eq!(bucket_name("otlpflare-test05"), "otlpflare-test05");
    }

    #[test]
    fn test_bucket_name_without_prefix() {
        assert_eq!(bucket_name("test05"), "otlpflare-test05");
    }

    #[test]
    fn test_stream_name_with_prefix() {
        assert_eq!(
            stream_name("otlpflare-test05", "logs"),
            "otlpflare_test05_logs"
        );
    }
}
