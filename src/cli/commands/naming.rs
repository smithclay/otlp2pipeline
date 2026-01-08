/// Normalize environment name by stripping frostbit prefix if present
pub fn normalize(name: &str) -> &str {
    name.strip_prefix("frostbit-")
        .or_else(|| name.strip_prefix("frostbit_"))
        .unwrap_or(name)
}

/// Normalize environment name to use hyphens (for bucket/worker names)
fn normalize_with_hyphens(env: &str) -> String {
    normalize(env).replace('_', "-")
}

/// Normalize environment name to use underscores (for stream/sink/pipeline names)
fn normalize_with_underscores(env: &str) -> String {
    normalize(env).replace('-', "_")
}

pub fn bucket_name(env: &str) -> String {
    format!("frostbit-{}", normalize_with_hyphens(env))
}

pub fn stream_name(env: &str, signal: &str) -> String {
    format!("frostbit_{}_{}", normalize_with_underscores(env), signal)
}

pub fn sink_name(env: &str, signal: &str) -> String {
    format!(
        "frostbit_{}_{}_sink",
        normalize_with_underscores(env),
        signal
    )
}

pub fn pipeline_name(env: &str, signal: &str) -> String {
    format!("frostbit_{}_{}", normalize_with_underscores(env), signal)
}

pub fn worker_name(env: &str) -> String {
    format!("frostbit-{}", normalize_with_hyphens(env))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_strips_dash_prefix() {
        assert_eq!(normalize("frostbit-test05"), "test05");
    }

    #[test]
    fn test_normalize_strips_underscore_prefix() {
        assert_eq!(normalize("frostbit_test05"), "test05");
    }

    #[test]
    fn test_normalize_no_prefix() {
        assert_eq!(normalize("test05"), "test05");
    }

    #[test]
    fn test_bucket_name_with_prefix() {
        assert_eq!(bucket_name("frostbit-test05"), "frostbit-test05");
    }

    #[test]
    fn test_bucket_name_without_prefix() {
        assert_eq!(bucket_name("test05"), "frostbit-test05");
    }

    #[test]
    fn test_stream_name_with_prefix() {
        assert_eq!(
            stream_name("frostbit-test05", "logs"),
            "frostbit_test05_logs"
        );
    }

    #[test]
    fn test_worker_name_with_prefix() {
        assert_eq!(worker_name("frostbit-test05"), "frostbit-test05");
    }

    #[test]
    fn test_worker_name_without_prefix() {
        assert_eq!(worker_name("test05"), "frostbit-test05");
    }
}
