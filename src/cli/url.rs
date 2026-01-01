use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

/// Resolve worker URL from explicit flag or wrangler.toml
pub fn resolve_worker_url(explicit_url: Option<&str>) -> Result<String> {
    if let Some(url) = explicit_url {
        return Ok(url.trim_end_matches('/').to_string());
    }

    // Try to parse wrangler.toml
    let wrangler_path = Path::new("wrangler.toml");
    if !wrangler_path.exists() {
        bail!(
            "No --url provided and wrangler.toml not found.\n\n\
            Either:\n  \
            1. Provide --url https://your-worker.workers.dev\n  \
            2. Run from a directory with wrangler.toml containing routes"
        );
    }

    let content = fs::read_to_string(wrangler_path)?;
    let config: toml::Value = toml::from_str(&content)?;

    resolve_url_from_config(&config)
}

fn extract_url_from_pattern(pattern: &str) -> Result<String> {
    // Pattern could be: "example.com/*" or "https://example.com/*"
    let pattern = pattern.trim_end_matches("/*").trim_end_matches("*");
    let pattern = pattern.trim_end_matches('/');

    if pattern.starts_with("http://") || pattern.starts_with("https://") {
        Ok(pattern.to_string())
    } else {
        Ok(format!("https://{}", pattern))
    }
}

/// Resolve URL from parsed wrangler.toml config (for testing)
pub fn resolve_url_from_config(config: &toml::Value) -> Result<String> {
    // Try routes array first (most common)
    if let Some(routes) = config.get("routes").and_then(|r| r.as_array()) {
        if let Some(first_route) = routes.first() {
            if let Some(pattern) = first_route.get("pattern").and_then(|p| p.as_str()) {
                return extract_url_from_pattern(pattern);
            }
            // Simple string route
            if let Some(pattern) = first_route.as_str() {
                return extract_url_from_pattern(pattern);
            }
        }
    }

    // Try route string (legacy format)
    if let Some(route) = config.get("route").and_then(|r| r.as_str()) {
        return extract_url_from_pattern(route);
    }

    // Try workers.dev subdomain from name
    if let Some(name) = config.get("name").and_then(|n| n.as_str()) {
        return Ok(format!("https://{}.workers.dev", name));
    }

    bail!(
        "Could not determine worker URL from wrangler.toml.\n\n\
        Please provide --url explicitly."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_url_returned_as_is() {
        let result = resolve_worker_url(Some("https://my-worker.workers.dev")).unwrap();
        assert_eq!(result, "https://my-worker.workers.dev");
    }

    #[test]
    fn test_explicit_url_trailing_slash_trimmed() {
        let result = resolve_worker_url(Some("https://my-worker.workers.dev/")).unwrap();
        assert_eq!(result, "https://my-worker.workers.dev");
    }

    #[test]
    fn test_extract_url_from_pattern_with_wildcard() {
        let result = extract_url_from_pattern("example.com/*").unwrap();
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn test_extract_url_from_pattern_with_https() {
        let result = extract_url_from_pattern("https://example.com/*").unwrap();
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn test_extract_url_from_pattern_with_http() {
        let result = extract_url_from_pattern("http://localhost:8787/*").unwrap();
        assert_eq!(result, "http://localhost:8787");
    }

    #[test]
    fn test_extract_url_from_pattern_bare_domain() {
        let result = extract_url_from_pattern("api.example.com").unwrap();
        assert_eq!(result, "https://api.example.com");
    }

    #[test]
    fn test_resolve_from_routes_array_with_pattern() {
        let config: toml::Value = toml::from_str(
            r#"
            [[routes]]
            pattern = "api.example.com/*"
            "#,
        )
        .unwrap();
        let result = resolve_url_from_config(&config).unwrap();
        assert_eq!(result, "https://api.example.com");
    }

    #[test]
    fn test_resolve_from_routes_array_string() {
        let config: toml::Value = toml::from_str(
            r#"
            routes = ["example.com/*"]
            "#,
        )
        .unwrap();
        let result = resolve_url_from_config(&config).unwrap();
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn test_resolve_from_legacy_route() {
        let config: toml::Value = toml::from_str(
            r#"
            route = "legacy.example.com/*"
            "#,
        )
        .unwrap();
        let result = resolve_url_from_config(&config).unwrap();
        assert_eq!(result, "https://legacy.example.com");
    }

    #[test]
    fn test_resolve_from_name_field() {
        let config: toml::Value = toml::from_str(
            r#"
            name = "my-worker"
            "#,
        )
        .unwrap();
        let result = resolve_url_from_config(&config).unwrap();
        assert_eq!(result, "https://my-worker.workers.dev");
    }

    #[test]
    fn test_resolve_priority_routes_over_name() {
        let config: toml::Value = toml::from_str(
            r#"
            name = "my-worker"
            [[routes]]
            pattern = "custom.example.com/*"
            "#,
        )
        .unwrap();
        let result = resolve_url_from_config(&config).unwrap();
        assert_eq!(result, "https://custom.example.com");
    }

    #[test]
    fn test_resolve_fails_with_empty_config() {
        let config: toml::Value = toml::from_str(
            r#"
            compatibility_date = "2024-01-01"
            "#,
        )
        .unwrap();
        let result = resolve_url_from_config(&config);
        assert!(result.is_err());
    }
}
