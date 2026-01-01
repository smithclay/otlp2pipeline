//! Worker-local cache for livetail client presence.
//!
//! Tracks which {service}:{signal} DOs have connected clients to avoid
//! unnecessary DO calls during ingestion. Uses a shorter TTL (10s) than
//! registry cache since client presence changes more frequently.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static LIVETAIL_CACHE: RefCell<LiveTailCache> = RefCell::new(LiveTailCache::default());
}

/// In-memory cache of DO client presence with per-key TTL.
#[derive(Default)]
struct LiveTailCache {
    /// Map of {service}:{signal} -> has_clients
    presence: HashMap<String, bool>,
    /// Per-key last refresh timestamp in milliseconds.
    last_refresh_ms: HashMap<String, u64>,
}

impl LiveTailCache {
    /// Cache TTL in milliseconds (10 seconds).
    /// Shorter than registry (3 min) since client presence changes frequently.
    const TTL_MS: u64 = 10_000;

    /// Check if a key's cache entry is fresh.
    fn is_fresh(&self, do_name: &str, now_ms: u64) -> bool {
        match self.last_refresh_ms.get(do_name) {
            Some(&last) => now_ms.saturating_sub(last) < Self::TTL_MS,
            None => false,
        }
    }
}

/// Check if a DO has clients according to the cache.
///
/// Returns:
/// - `Some(true)` if DO has clients (cache hit, fresh)
/// - `Some(false)` if DO has no clients (cache hit, fresh)
/// - `None` if cache miss or stale (requires DO call)
pub fn has_clients(do_name: &str) -> Option<bool> {
    LIVETAIL_CACHE.with(|cache| {
        let cache = cache.borrow();
        let now_ms = current_time_ms();

        if cache.is_fresh(do_name, now_ms) {
            cache.presence.get(do_name).copied()
        } else {
            None
        }
    })
}

/// Update the cache with client presence for a DO.
///
/// Call this after receiving a response from the DO with client count.
pub fn update(do_name: &str, has_clients: bool) {
    LIVETAIL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let now_ms = current_time_ms();
        cache.presence.insert(do_name.to_string(), has_clients);
        cache.last_refresh_ms.insert(do_name.to_string(), now_ms);
    });
}

/// Get current time in milliseconds since epoch.
#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> u64 {
    worker::Date::now().as_millis()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_clients_cache_miss() {
        // Unknown key returns None
        assert!(has_clients("unknown:logs").is_none());
    }

    #[test]
    fn test_update_and_has_clients() {
        let do_name = "test-service:logs";

        // Update with has_clients = true
        update(do_name, true);
        assert_eq!(has_clients(do_name), Some(true));

        // Update with has_clients = false
        update(do_name, false);
        assert_eq!(has_clients(do_name), Some(false));
    }

    #[test]
    fn test_cache_freshness() {
        let cache = LiveTailCache {
            presence: HashMap::new(),
            last_refresh_ms: [("test:logs".to_string(), 1000)].into_iter().collect(),
        };

        // Fresh if within TTL
        assert!(cache.is_fresh("test:logs", 1000 + LiveTailCache::TTL_MS - 1));

        // Stale if at or past TTL
        assert!(!cache.is_fresh("test:logs", 1000 + LiveTailCache::TTL_MS));
        assert!(!cache.is_fresh("test:logs", 1000 + LiveTailCache::TTL_MS + 1000));
    }

    #[test]
    fn test_cache_unknown_key_not_fresh() {
        let cache = LiveTailCache::default();
        assert!(!cache.is_fresh("unknown:logs", current_time_ms()));
    }
}
