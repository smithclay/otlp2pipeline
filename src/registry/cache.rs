//! Worker-local cache for service registry to minimize Durable Object calls.

use std::cell::RefCell;
use std::collections::HashSet;

thread_local! {
    static REGISTRY_CACHE: RefCell<RegistryCache> = RefCell::new(RegistryCache::default());
}

/// In-memory cache of known services with TTL.
#[derive(Default)]
struct RegistryCache {
    /// Set of known service names.
    services: HashSet<String>,
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

/// Check if a service is known in the local cache.
///
/// O(1) HashSet lookup. Returns false if service not in cache
/// (doesn't mean service doesn't exist in DO).
pub fn is_known(service_name: &str) -> bool {
    REGISTRY_CACHE.with(|cache| {
        let cache = cache.borrow();
        cache.services.contains(service_name)
    })
}

/// Add a service to the local cache.
///
/// Call this when a new service is discovered during ingestion.
/// The caller is responsible for queuing the DO write.
pub fn add_locally(service_name: String) {
    REGISTRY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.services.insert(service_name);
    });
}

/// Get all cached services if the cache is fresh (< 3 minutes old).
///
/// Returns None if cache is stale or empty, requiring a refresh from DO.
pub fn get_all_if_fresh() -> Option<Vec<String>> {
    REGISTRY_CACHE.with(|cache| {
        let cache = cache.borrow();
        let now_ms = current_time_ms();

        if cache.is_fresh(now_ms) && !cache.services.is_empty() {
            Some(cache.services.iter().cloned().collect())
        } else {
            None
        }
    })
}

/// Refresh the cache with a fresh list from the Durable Object.
///
/// Replaces the entire cache and resets the TTL.
pub fn refresh(services: Vec<String>) {
    REGISTRY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.services = services.into_iter().collect();
        cache.last_refresh_ms = current_time_ms();
    });
}

/// Get current time in milliseconds since epoch.
///
/// Uses worker::Date::now() on WASM, SystemTime on native.
#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> u64 {
    worker::Date::now().as_millis() as u64
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
        assert!(!is_known("service1"));
    }

    #[test]
    fn test_add_locally() {
        add_locally("test_service".to_string());
        assert!(is_known("test_service"));
        assert!(!is_known("other_service"));
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

        // All services should be known
        assert!(is_known("svc1"));
        assert!(is_known("svc2"));
        assert!(is_known("svc3"));
    }

    #[test]
    fn test_cache_freshness() {
        let cache = RegistryCache {
            services: HashSet::new(),
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
