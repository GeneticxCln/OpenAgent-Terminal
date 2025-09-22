use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, Instant};
use tracing::{error, info, warn};

/// Standard timeout durations used throughout the application
pub struct Timeouts;

impl Timeouts {
    /// Short operations like file I/O, config reads
    pub const SHORT: Duration = Duration::from_millis(1000);
    /// Medium operations like plugin loading, API calls
    pub const MEDIUM: Duration = Duration::from_millis(5000);
    /// Long operations like AI model loading, large file processing
    pub const LONG: Duration = Duration::from_millis(30000);
    /// Network requests (HTTP/HTTPS)
    pub const NETWORK: Duration = Duration::from_millis(10000);
    /// Database operations
    pub const DATABASE: Duration = Duration::from_millis(5000);
    /// WASM plugin operations
    pub const PLUGIN: Duration = Duration::from_millis(3000);
}

/// Standard timeout error type
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Operation timed out after {timeout:?} in {operation}")]
    Timeout { timeout: Duration, operation: String },

    #[error("Operation cancelled: {reason}")]
    Cancelled { reason: String },
}

/// Execute a future with a timeout, logging on timeout
pub async fn timeout_with_log<T, F>(
    future: F,
    timeout_duration: Duration,
    operation_name: &str,
) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    let start = Instant::now();

    match timeout(timeout_duration, future).await {
        Ok(result) => {
            let elapsed = start.elapsed();
            if elapsed > timeout_duration / 2 {
                warn!(
                    "Operation '{}' took {:?}, close to timeout {:?}",
                    operation_name, elapsed, timeout_duration
                );
            }
            Ok(result)
        }
        Err(_) => {
            error!("Operation '{}' timed out after {:?}", operation_name, timeout_duration);
            Err(TimeoutError::Timeout {
                timeout: timeout_duration,
                operation: operation_name.to_string(),
            })
        }
    }
}

/// Execute a future with cancellation token support
pub async fn with_cancellation<T, F>(
    future: F,
    cancellation_token: tokio_util::sync::CancellationToken,
    operation_name: &str,
) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    tokio::select! {
        result = future => Ok(result),
        _ = cancellation_token.cancelled() => {
            warn!("Operation '{}' was cancelled", operation_name);
            Err(TimeoutError::Cancelled {
                reason: format!("Operation '{}' was cancelled", operation_name),
            })
        }
    }
}

/// Execute a future with both timeout and cancellation support
pub async fn timeout_with_cancellation<T, F>(
    future: F,
    timeout_duration: Duration,
    cancellation_token: tokio_util::sync::CancellationToken,
    operation_name: &str,
) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    let start = Instant::now();

    tokio::select! {
        result = timeout(timeout_duration, future) => {
            match result {
                Ok(value) => {
                    let elapsed = start.elapsed();
                    if elapsed > timeout_duration / 2 {
                        warn!(
                            "Operation '{}' took {:?}, close to timeout {:?}",
                            operation_name, elapsed, timeout_duration
                        );
                    }
                    Ok(value)
                }
                Err(_) => {
                    error!(
                        "Operation '{}' timed out after {:?}",
                        operation_name, timeout_duration
                    );
                    Err(TimeoutError::Timeout {
                        timeout: timeout_duration,
                        operation: operation_name.to_string(),
                    })
                }
            }
        }
        _ = cancellation_token.cancelled() => {
            warn!("Operation '{}' was cancelled after {:?}", operation_name, start.elapsed());
            Err(TimeoutError::Cancelled {
                reason: format!("Operation '{}' was cancelled", operation_name),
            })
        }
    }
}

/// Retry a future operation with exponential backoff
pub async fn retry_with_backoff<T, E, F, Fut>(
    mut operation: F,
    max_attempts: u32,
    initial_delay: Duration,
    operation_name: &str,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempts = 0;
    let mut delay = initial_delay;

    loop {
        attempts += 1;

        match operation().await {
            Ok(result) => {
                if attempts > 1 {
                    warn!(
                        "Operation '{}' succeeded on attempt {}/{}",
                        operation_name, attempts, max_attempts
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                if attempts >= max_attempts {
                    error!(
                        "Operation '{}' failed after {} attempts: {:?}",
                        operation_name, max_attempts, e
                    );
                    return Err(e);
                }

                warn!(
                    "Operation '{}' failed on attempt {}/{}, retrying in {:?}: {:?}",
                    operation_name, attempts, max_attempts, delay, e
                );

                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(60)); // Cap at 60 seconds
            }
        }
    }
}

/// Create a graceful shutdown handler that waits for operations to complete
pub async fn graceful_shutdown<F>(
    shutdown_signal: F,
    active_operations: Arc<tokio::sync::RwLock<u32>>,
    max_shutdown_wait: Duration,
) where
    F: Future<Output = ()>,
{
    shutdown_signal.await;

    let start = Instant::now();
    info!("Graceful shutdown initiated, waiting for operations to complete...");

    while start.elapsed() < max_shutdown_wait {
        let count = *active_operations.read().await;
        if count == 0 {
            info!("All operations completed, shutting down");
            return;
        }

        warn!("Waiting for {} active operations to complete", count);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let remaining = *active_operations.read().await;
    if remaining > 0 {
        error!(
            "Forcing shutdown with {} operations still active after {:?}",
            remaining, max_shutdown_wait
        );
    }
}

/// Operation guard that automatically decrements the active operation counter
pub struct OperationGuard {
    counter: Arc<tokio::sync::RwLock<u32>>,
    name: String,
}

impl OperationGuard {
    pub async fn new(counter: Arc<tokio::sync::RwLock<u32>>, operation_name: String) -> Self {
        {
            let mut count = counter.write().await;
            *count += 1;
        }

        Self { counter, name: operation_name }
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        let counter = self.counter.clone();
        let name = self.name.clone();

        tokio::spawn(async move {
            let mut count = counter.write().await;
            if *count > 0 {
                *count -= 1;
            } else {
                error!("Operation counter underflow for operation: {}", name);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_timeout_success() {
        let result = timeout_with_log(async { "success" }, Timeouts::SHORT, "test_operation").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_timeout_failure() {
        let result = timeout_with_log(
            async {
                sleep(Duration::from_millis(2000)).await;
                "should not reach here"
            },
            Duration::from_millis(100),
            "test_timeout",
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancellation() {
        let token = tokio_util::sync::CancellationToken::new();
        let token_clone = token.clone();

        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            token_clone.cancel();
        });

        let result = with_cancellation(
            async {
                sleep(Duration::from_millis(1000)).await;
                "should not reach here"
            },
            token,
            "test_cancellation",
        )
        .await;

        assert!(result.is_err());
        if let Err(TimeoutError::Cancelled { .. }) = result {
            // Expected
        } else {
            panic!("Expected cancellation error");
        }
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let mut attempt_count = 0;

        let result = retry_with_backoff(
            || {
                attempt_count += 1;
                async move {
                    if attempt_count < 3 {
                        Err("temporary error")
                    } else {
                        Ok("success")
                    }
                }
            },
            5,
            Duration::from_millis(10),
            "test_retry",
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count, 3);
    }

    #[tokio::test]
    async fn test_operation_guard() {
        let counter = Arc::new(tokio::sync::RwLock::new(0u32));

        {
            let _guard = OperationGuard::new(counter.clone(), "test_op".to_string()).await;
            assert_eq!(*counter.read().await, 1);
        }

        // Give the drop handler a chance to run
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(*counter.read().await, 0);
    }
}
