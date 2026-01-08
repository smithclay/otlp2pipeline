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
    service_signals: HashSet<(String, String)>,
    /// Set of known service names (for list queries).
    service_names: HashSet<String>,
    /// Last refresh timestamp for services in milliseconds since epoch.
    last_refresh_ms: u64,
    /// Set of known (metric_name, metric_type) tuples.
    metric_types: HashSet<(String, String)>,
    /// Last refresh timestamp for metrics in milliseconds since epoch.
    last_metrics_refresh_ms: u64,
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

    /// Check if metrics cache is fresh (< 3 minutes old).
    fn is_metrics_fresh(&self, now_ms: u64) -> bool {
        if self.last_metrics_refresh_ms == 0 {
            return false;
        }
        now_ms.saturating_sub(self.last_metrics_refresh_ms) < Self::TTL_MS
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

/// Check if a metric (name, type) is known in the local cache.
pub fn is_metric_known(name: &str, metric_type: &str) -> bool {
    REGISTRY_CACHE.with(|cache| {
        let cache = cache.borrow();
        cache
            .metric_types
            .contains(&(name.to_string(), metric_type.to_string()))
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

/// Add a metric (name, type) to the local cache.
pub fn add_metric_locally(name: String, metric_type: String) {
    REGISTRY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.metric_types.insert((name, metric_type));
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

/// Get all cached metrics if the cache is fresh (< 3 minutes old).
pub fn get_all_metrics_if_fresh() -> Option<Vec<(String, String)>> {
    REGISTRY_CACHE.with(|cache| {
        let cache = cache.borrow();
        let now_ms = current_time_ms();

        if cache.is_metrics_fresh(now_ms) && !cache.metric_types.is_empty() {
            Some(cache.metric_types.iter().cloned().collect())
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

/// Refresh the metrics cache with a fresh list from the Durable Object.
pub fn refresh_metrics(metrics: Vec<(String, String)>) {
    REGISTRY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.metric_types = metrics.into_iter().collect();
        cache.last_metrics_refresh_ms = current_time_ms();
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
            metric_types: HashSet::new(),
            last_metrics_refresh_ms: 0,
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

    #[test]
    fn test_is_metric_known_empty_cache() {
        assert!(!is_metric_known("http_requests", "sum"));
    }

    #[test]
    fn test_add_metric_locally() {
        add_metric_locally("cpu_usage".to_string(), "gauge".to_string());
        assert!(is_metric_known("cpu_usage", "gauge"));
        // Same name with different type should NOT be known
        assert!(!is_metric_known("cpu_usage", "sum"));
    }

    #[test]
    fn test_same_metric_multiple_types() {
        add_metric_locally("requests".to_string(), "sum".to_string());
        add_metric_locally("requests".to_string(), "histogram".to_string());

        assert!(is_metric_known("requests", "sum"));
        assert!(is_metric_known("requests", "histogram"));
        assert!(!is_metric_known("requests", "gauge"));
    }

    #[test]
    fn test_refresh_metrics() {
        let metrics = vec![
            ("m1".to_string(), "gauge".to_string()),
            ("m2".to_string(), "sum".to_string()),
        ];
        refresh_metrics(metrics);

        let cached = get_all_metrics_if_fresh();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 2);
    }

    #[test]
    fn test_metrics_cache_freshness() {
        let cache = RegistryCache {
            service_signals: HashSet::new(),
            service_names: HashSet::new(),
            last_refresh_ms: 0,
            metric_types: HashSet::new(),
            last_metrics_refresh_ms: 1000,
        };

        assert!(cache.is_metrics_fresh(1000 + RegistryCache::TTL_MS - 1));
        assert!(!cache.is_metrics_fresh(1000 + RegistryCache::TTL_MS));
    }
}
