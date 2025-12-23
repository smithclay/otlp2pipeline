use std::future::Future;
use std::time::Duration;

/// Retry configuration
#[derive(Clone, Debug)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3, // 1 initial + 2 retries
            delay: Duration::from_millis(500),
        }
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
                tracing::debug!(
                    attempt = attempt + 1,
                    max = attempts,
                    "retrying after transient error"
                );
                last_error = Some(e);
                sleep(config.delay).await;
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
