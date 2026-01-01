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
