//! Worker-local cache for service registry to minimize Durable Object calls.

use std::cell::RefCell;
use std::collections::HashSet;

thread_local! {
    static REGISTRY_CACHE: RefCell<RegistryCache> = RefCell::new(RegistryCache::default());
}

/// In-memory cache of known services with TTL.
/// Tracks (service_name, signal) tuples to allow different signal types
/// for the same service to reach the Durable Object.
#[derive(Default)]
struct RegistryCache {
    /// Set of known (service_name, signal) tuples.
    /// Signal is stored as a string: "logs", "traces", or "metrics".
    service_signals: HashSet<(String, String)>,
    /// Set of known service names (for list queries).
    service_names: HashSet<String>,
    /// Last refresh timestamp in milliseconds since epoch.
    last_refresh_ms: u64,
}

impl RegistryCache {
    /// Cache TTL in milliseconds (3 minutes).
    const TTL_MS: u64 = 3 * 60 * 1000;

    /// Check if cache is fresh (< 3 minutes old).
    fn is_fresh(&self, now_ms: u64) -> bool {
        if self.last_refresh_ms == 0 {
            return false; // Never refreshed
        }
        now_ms.saturating_sub(self.last_refresh_ms) < Self::TTL_MS
    }
}

/// Check if a service+signal combination is known in the local cache.
///
/// O(1) HashSet lookup. Returns false if combination not in cache
/// (doesn't mean it doesn't exist in DO).
pub fn is_known(service_name: &str, signal: &str) -> bool {
    REGISTRY_CACHE.with(|cache| {
        let cache = cache.borrow();
        cache
            .service_signals
            .contains(&(service_name.to_string(), signal.to_string()))
    })
}

/// Add a service+signal combination to the local cache.
///
/// Call this when a new service is discovered during ingestion.
/// The caller is responsible for queuing the DO write.
pub fn add_locally(service_name: String, signal: String) {
    REGISTRY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.service_signals.insert((service_name.clone(), signal));
        cache.service_names.insert(service_name);
    });
}

/// Get all cached service names if the cache is fresh (< 3 minutes old).
///
/// Returns None if cache is stale or empty, requiring a refresh from DO.
pub fn get_all_if_fresh() -> Option<Vec<String>> {
    REGISTRY_CACHE.with(|cache| {
        let cache = cache.borrow();
        let now_ms = current_time_ms();

        if cache.is_fresh(now_ms) && !cache.service_names.is_empty() {
            Some(cache.service_names.iter().cloned().collect())
        } else {
            None
        }
    })
}

/// Refresh the cache with a fresh list from the Durable Object.
///
/// Replaces the entire cache and resets the TTL.
/// Note: This only refreshes service names; signal info comes from DO queries.
pub fn refresh(services: Vec<String>) {
    REGISTRY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.service_names = services.into_iter().collect();
        // Clear service_signals since we don't have signal info from list query
        // This ensures new signals will be registered on next ingestion
        cache.service_signals.clear();
        cache.last_refresh_ms = current_time_ms();
    });
}

/// Get current time in milliseconds since epoch.
///
/// Uses worker::Date::now() on WASM, SystemTime on native.
#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> u64 {
    worker::Date::now().as_millis()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_empty_cache() {
        assert!(!is_known("service1", "logs"));
    }

    #[test]
    fn test_add_locally_with_signal() {
        add_locally("test_service".to_string(), "logs".to_string());
        assert!(is_known("test_service", "logs"));
        // Same service with different signal should NOT be known
        assert!(!is_known("test_service", "metrics"));
        assert!(!is_known("other_service", "logs"));
    }

    #[test]
    fn test_multiple_signals_same_service() {
        add_locally("my_app".to_string(), "logs".to_string());
        add_locally("my_app".to_string(), "traces".to_string());
        add_locally("my_app".to_string(), "metrics".to_string());

        // All combinations should be known
        assert!(is_known("my_app", "logs"));
        assert!(is_known("my_app", "traces"));
        assert!(is_known("my_app", "metrics"));
    }

    #[test]
    fn test_get_all_if_fresh_empty() {
        // Empty cache should return None
        assert!(get_all_if_fresh().is_none());
    }

    #[test]
    fn test_refresh_and_get_all() {
        let services = vec!["svc1".to_string(), "svc2".to_string(), "svc3".to_string()];
        refresh(services.clone());

        // Should be fresh immediately after refresh
        let cached = get_all_if_fresh();
        assert!(cached.is_some());

        let mut cached = cached.unwrap();
        cached.sort();
        let mut expected = services.clone();
        expected.sort();
        assert_eq!(cached, expected);
    }

    #[test]
    fn test_refresh_clears_signal_cache() {
        // Add a service with signal
        add_locally("svc1".to_string(), "logs".to_string());
        assert!(is_known("svc1", "logs"));

        // Refresh with service list (simulating DO query)
        refresh(vec!["svc1".to_string()]);

        // Signal cache should be cleared, so is_known returns false
        // This allows re-registration of signals after cache refresh
        assert!(!is_known("svc1", "logs"));
    }

    #[test]
    fn test_cache_freshness() {
        let cache = RegistryCache {
            service_signals: HashSet::new(),
            service_names: HashSet::new(),
            last_refresh_ms: 1000,
        };

        // Fresh if within TTL
        assert!(cache.is_fresh(1000 + RegistryCache::TTL_MS - 1));

        // Stale if at or past TTL
        assert!(!cache.is_fresh(1000 + RegistryCache::TTL_MS));
        assert!(!cache.is_fresh(1000 + RegistryCache::TTL_MS + 1000));
    }

    #[test]
    fn test_cache_never_refreshed() {
        let cache = RegistryCache::default();
        assert!(!cache.is_fresh(current_time_ms()));
    }
}
