use std::future::Future;
use std::time::Duration;

/// Backoff strategy for retries
#[derive(Clone, Debug, Default)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    #[default]
    Fixed,
    /// Exponential backoff with jitter: delay = min(base * 2^attempt + jitter, max)
    /// Used by Lambda for AWS API compatibility.
    #[allow(dead_code)] // Used with lambda feature
    ExponentialWithJitter { base_ms: u64, max_ms: u64 },
}

/// Retry configuration
#[derive(Clone, Debug)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub delay: Duration,
    pub backoff: BackoffStrategy,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3, // 1 initial + 2 retries
            delay: Duration::from_millis(500),
            backoff: BackoffStrategy::Fixed,
        }
    }
}

impl RetryConfig {
    /// Create a config with exponential backoff and jitter (recommended for AWS APIs)
    #[allow(dead_code)] // Used with lambda feature
    pub fn exponential(max_attempts: u32, base_ms: u64, max_ms: u64) -> Self {
        Self {
            max_attempts,
            delay: Duration::from_millis(base_ms), // Used as base for exponential
            backoff: BackoffStrategy::ExponentialWithJitter { base_ms, max_ms },
        }
    }

    /// Calculate delay for a given attempt number
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        match &self.backoff {
            BackoffStrategy::Fixed => self.delay,
            BackoffStrategy::ExponentialWithJitter { base_ms, max_ms } => {
                let base = base_ms.saturating_mul(2_u64.saturating_pow(attempt));
                let jitter = random_jitter(base / 2);
                let total = base.saturating_add(jitter).min(*max_ms);
                Duration::from_millis(total)
            }
        }
    }
}

/// Generate random jitter up to max_jitter
fn random_jitter(max_jitter: u64) -> u64 {
    if max_jitter == 0 {
        return 0;
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Use js_sys for WASM randomness
        (js_sys::Math::random() * max_jitter as f64) as u64
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "lambda"))]
    {
        use rand::Rng;
        rand::thread_rng().gen_range(0..=max_jitter)
    }
    // For native builds without lambda feature (tests), use deterministic half of max
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "lambda")))]
    {
        max_jitter / 2
    }
}

/// Trait for errors that may be retryable
pub trait IsRetryable {
    fn is_retryable(&self) -> bool;
}

/// Execute an async operation with retries.
/// Only retries on transient errors (as determined by IsRetryable trait).
pub async fn with_retry<F, Fut, T, E>(config: &RetryConfig, mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: IsRetryable,
{
    let attempts = config.max_attempts.max(1);
    let mut last_error: Option<E> = None;

    for attempt in 0..attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_retryable() && attempt + 1 < attempts => {
                let delay = config.delay_for_attempt(attempt);
                tracing::debug!(
                    attempt = attempt + 1,
                    max = attempts,
                    delay_ms = delay.as_millis() as u64,
                    "retrying after transient error"
                );
                last_error = Some(e);
                sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }

    Err(last_error.expect("retry loop should have returned an error"))
}

// Platform-specific sleep implementation
#[cfg(target_arch = "wasm32")]
async fn sleep(duration: Duration) {
    gloo_timers::future::TimeoutFuture::new(duration.as_millis() as u32).await;
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestError {
        retryable: bool,
    }

    impl IsRetryable for TestError {
        fn is_retryable(&self) -> bool {
            self.retryable
        }
    }

    #[tokio::test]
    async fn succeeds_on_first_attempt() {
        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let count = call_count.clone();

        let result: Result<&str, TestError> = with_retry(&config, || {
            count.fetch_add(1, Ordering::SeqCst);
            async { Ok("success") }
        })
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_on_transient_error() {
        let config = RetryConfig {
            max_attempts: 3,
            delay: Duration::from_millis(1), // fast for tests
            backoff: BackoffStrategy::Fixed,
        };
        let call_count = Arc::new(AtomicU32::new(0));
        let count = call_count.clone();

        let result: Result<&str, TestError> = with_retry(&config, || {
            let attempt = count.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt < 2 {
                    Err(TestError { retryable: true })
                } else {
                    Ok("success after retries")
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "success after retries");
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable_error() {
        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let count = call_count.clone();

        let result: Result<&str, TestError> = with_retry(&config, || {
            count.fetch_add(1, Ordering::SeqCst);
            async { Err(TestError { retryable: false }) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn zero_attempts_are_clamped_to_one() {
        let config = RetryConfig {
            max_attempts: 0,
            delay: Duration::from_millis(1),
            backoff: BackoffStrategy::Fixed,
        };
        let call_count = Arc::new(AtomicU32::new(0));
        let count = call_count.clone();

        let result: Result<&str, TestError> = with_retry(&config, || {
            count.fetch_add(1, Ordering::SeqCst);
            async { Err(TestError { retryable: true }) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
