use crate::error::AppError;
use reqwest::{Response, StatusCode};
use std::time::Duration;

/// Conservative retry policy for provider HTTP calls.
///
/// Only transient transport failures and the handful of retryable server
/// statuses are retried. Authentication, validation and schema errors are never
/// retried, because doing so cannot make them succeed.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(250),
        }
    }
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            base_delay: Duration::ZERO,
        }
    }
}

pub fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::REQUEST_TIMEOUT
    )
}

fn should_retry_network(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_connect() {
        return true;
    }
    let text = error.to_string().to_ascii_lowercase();
    text.contains("connection reset")
        || text.contains("connection closed")
        || text.contains("connection refused")
        || text.contains("temporarily unavailable")
        || text.contains("timed out")
}

fn retry_delay(policy: RetryPolicy, attempt: u32) -> Duration {
    let exponent = 1u64 << attempt.min(6);
    let base = policy.base_delay.saturating_mul(exponent as u32);
    // Deterministic-enough jitter without pulling in a random number crate.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64)
        .unwrap_or(0);
    base + Duration::from_millis(nanos % 125)
}

/// Send a request, retrying transient failures.
///
/// `make_request` must build a fresh `RequestBuilder` each call. Request bodies
/// are not buffered or replayed by this helper.
pub async fn send_with_retry<F>(
    mut make_request: F,
    policy: RetryPolicy,
) -> Result<Response, AppError>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut attempt = 0u32;
    loop {
        let response = make_request().send().await;
        match response {
            Ok(response) => {
                let status = response.status();
                if is_retryable_status(status) && attempt < policy.max_retries {
                    let mut delay = retry_delay(policy, attempt);
                    // Rate limits need time to clear; rapid retries consume the
                    // same constrained slot without giving other workers time.
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        delay = delay.max(Duration::from_secs(5 * (1 << attempt.min(3))));
                    }
                    if let Some(seconds) = response.headers().get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()) {
                        // Surface long cooldowns instead of retrying before the
                        // provider permits it or parking a worker indefinitely.
                        if seconds > 60 { return Ok(response); }
                        delay = delay.max(Duration::from_secs(seconds));
                    }
                    tracing::warn!(
                        %status,
                        attempt = attempt + 1,
                        delay_ms = delay.as_millis(),
                        "provider returned a retryable status"
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                    continue;
                }
                return Ok(response);
            }
            Err(error) => {
                if should_retry_network(&error) && attempt < policy.max_retries {
                    let delay = retry_delay(policy, attempt);
                    tracing::warn!(
                        attempt = attempt + 1,
                        delay_ms = delay.as_millis(),
                        "provider network failure; retrying"
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                    continue;
                }
                return Err(AppError::Network(error.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_only_transient_statuses() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::FORBIDDEN));
        assert!(!is_retryable_status(StatusCode::UNPROCESSABLE_ENTITY));
    }

    #[test]
    fn backoff_grows_and_stays_bounded() {
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay: Duration::from_millis(100),
        };
        assert!(retry_delay(policy, 0) >= Duration::from_millis(100));
        assert!(retry_delay(policy, 1) >= Duration::from_millis(200));
    }
}
