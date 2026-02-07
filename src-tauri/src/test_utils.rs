//! Test utilities for async/daemon tests
//!
//! This module is only compiled in test builds and provides helpers
//! for writing reliable async tests with proper timeout/retry semantics.

use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

/// Assert that an async condition eventually becomes true within a timeout.
///
/// This helper retries the provided async function at the specified interval
/// until it succeeds or the timeout is reached. This avoids flaky tests that
/// use arbitrary `tokio::time::sleep()` values.
///
/// # Arguments
///
/// * `desc` - Human-readable description of what we're waiting for (for error messages)
/// * `timeout` - Maximum time to wait before failing
/// * `interval` - Time between retry attempts
/// * `f` - Async function that returns `Result<T, E>` where `Ok(_)` means success
///
/// # Example
///
/// ```rust,ignore
/// # use std::time::Duration;
/// # async fn example() {
/// use rn_desktop_2_lib::test_utils::assert_eventually;
///
/// // Wait up to 3 seconds for a session to stop, checking every 100ms
/// assert_eventually(
///     "session to stop",
///     Duration::from_secs(3),
///     Duration::from_millis(100),
///     || async {
///         let registry = get_registry().await;
///         match registry.get(session_id) {
///             Some(session) if session.status == SessionStatus::Stopped => Ok(()),
///             Some(_) => Err("session still running"),
///             None => Err("session not found"),
///         }
///     }
/// ).await;
/// # }
/// ```
pub async fn assert_eventually<F, Fut, T, E>(
    desc: &str,
    timeout: Duration,
    interval: Duration,
    mut f: F,
) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    let start = std::time::Instant::now();
    let mut last_error = String::from("no attempts made");
    let mut attempt = 0;

    loop {
        attempt += 1;
        match f().await {
            Ok(value) => return value,
            Err(e) => {
                last_error = e.to_string();

                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    panic!(
                        "Timeout waiting for {}\n\
                         Duration: {:?}\n\
                         Attempts: {}\n\
                         Last error: {}",
                        desc, elapsed, attempt, last_error
                    );
                }

                tokio::time::sleep(interval).await;
            }
        }
    }
}

/// Variant of `assert_eventually` that takes a simple boolean condition.
///
/// This is more ergonomic when you just want to wait for a boolean predicate
/// without needing a full Result type.
///
/// # Example
///
/// ```rust,ignore
/// # use std::time::Duration;
/// # async fn example() {
/// use rn_desktop_2_lib::test_utils::assert_eventually_bool;
///
/// assert_eventually_bool(
///     "tail to contain output",
///     Duration::from_secs(2),
///     Duration::from_millis(50),
///     || async {
///         let tail = get_tail().await;
///         tail.contains("expected-output")
///     }
/// ).await;
/// # }
/// ```
pub async fn assert_eventually_bool<F, Fut>(
    desc: &str,
    timeout: Duration,
    interval: Duration,
    mut f: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let start = std::time::Instant::now();
    let mut attempt = 0;

    loop {
        attempt += 1;
        if f().await {
            return;
        }

        let elapsed = start.elapsed();
        if elapsed >= timeout {
            panic!(
                "Timeout waiting for {}\n\
                 Duration: {:?}\n\
                 Attempts: {}\n\
                 Condition never became true",
                desc, elapsed, attempt
            );
        }

        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_assert_eventually_succeeds_immediately() {
        assert_eventually(
            "immediate success",
            Duration::from_secs(1),
            Duration::from_millis(50),
            || async { Ok::<_, &str>(42) },
        )
        .await;
    }

    #[tokio::test]
    async fn test_assert_eventually_succeeds_after_retries() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = assert_eventually(
            "counter to reach 3",
            Duration::from_secs(2),
            Duration::from_millis(50),
            move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    let val = c.fetch_add(1, Ordering::SeqCst);
                    if val >= 2 {
                        Ok(val)
                    } else {
                        Err(format!("counter only at {}", val))
                    }
                }
            },
        )
        .await;

        assert!(result >= 2);
    }

    #[tokio::test]
    #[should_panic(expected = "Timeout waiting for never succeeds")]
    async fn test_assert_eventually_times_out() {
        assert_eventually(
            "never succeeds",
            Duration::from_millis(200),
            Duration::from_millis(50),
            || async { Err::<(), _>("always fails") },
        )
        .await;
    }

    #[tokio::test]
    async fn test_assert_eventually_bool_true() {
        assert_eventually_bool(
            "always true",
            Duration::from_secs(1),
            Duration::from_millis(50),
            || async { true },
        )
        .await;
    }

    #[tokio::test]
    async fn test_assert_eventually_bool_retries() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        assert_eventually_bool(
            "counter reaches 2",
            Duration::from_secs(2),
            Duration::from_millis(50),
            move || {
                let c = Arc::clone(&counter_clone);
                async move { c.fetch_add(1, Ordering::SeqCst) >= 2 }
            },
        )
        .await;

        assert!(counter.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    #[should_panic(expected = "Timeout waiting for never true")]
    async fn test_assert_eventually_bool_times_out() {
        assert_eventually_bool(
            "never true",
            Duration::from_millis(200),
            Duration::from_millis(50),
            || async { false },
        )
        .await;
    }
}
