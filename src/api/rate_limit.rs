//! Per-account sliding window rate limiter.
//!
//! Uses an in-memory `DashMap<token, VecDeque<Instant>>` to track request
//! timestamps per bearer token. Returns 429 Too Many Requests with
//! `Retry-After` header when the per-minute limit is exceeded.

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Shared rate limiter state.
#[derive(Clone)]
pub struct RateLimiter {
    /// Per-token request timestamps within the current window.
    buckets: Arc<DashMap<String, VecDeque<Instant>>>,
    /// Maximum requests per minute. 0 = unlimited.
    limit: u64,
    /// Window duration (always 60s).
    window: Duration,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u64) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            limit: limit_per_minute,
            window: Duration::from_secs(60),
        }
    }

    /// Check if a request is allowed. Returns `Err(retry_after_secs)` if denied.
    fn check(&self, key: &str) -> Result<(), u64> {
        if self.limit == 0 {
            return Ok(());
        }

        let now = Instant::now();
        let cutoff = now - self.window;

        let mut entry = self.buckets.entry(key.to_string()).or_default();
        let timestamps = entry.value_mut();

        // Evict expired entries
        while timestamps.front().is_some_and(|&t| t < cutoff) {
            timestamps.pop_front();
        }

        if timestamps.len() as u64 >= self.limit {
            let oldest = timestamps.front().unwrap();
            let retry_after = self.window.saturating_sub(now - *oldest);
            Err(retry_after.as_secs().max(1))
        } else {
            timestamps.push_back(now);
            Ok(())
        }
    }
}

/// Axum middleware that enforces per-account rate limits.
///
/// Keyed by bearer token from the Authorization header. Unauthenticated
/// requests pass through (they'll be rejected by the auth extractor anyway).
pub async fn rate_limit_middleware(
    axum::Extension(limiter): axum::Extension<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    if let Some(token) = token
        && let Err(retry_after) = limiter.check(token)
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, retry_after.to_string())],
            axum::Json(serde_json::json!({
                "error": {
                    "code": 429,
                    "message": "rate limit exceeded",
                    "details": format!(
                        "too many requests, retry after {retry_after}s"
                    ),
                }
            })),
        )
            .into_response();
    }

    next.run(request).await
}
