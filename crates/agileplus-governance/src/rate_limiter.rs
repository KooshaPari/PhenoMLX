//! Rate limiting for governance operations
//!
//! Provides token bucket rate limiting per user/action to prevent abuse.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u64,
    /// Window size
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Rate limit key
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RateLimitKey {
    pub user_id: Option<String>,
    pub client_ip: Option<String>,
    pub action: String,
}

impl RateLimitKey {
    /// Create from request context
    pub fn new(
        user_id: Option<String>,
        client_ip: Option<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            user_id,
            client_ip,
            action: action.into(),
        }
    }

    /// Create for anonymous requests
    pub fn anonymous(client_ip: Option<String>, action: impl Into<String>) -> Self {
        Self {
            user_id: None,
            client_ip,
            action: action.into(),
        }
    }
}

/// Rate limit entry
#[derive(Debug, Clone)]
struct RateLimitEntry {
    tokens: u64,
    window_start: Instant,
}

impl RateLimitEntry {
    fn new(max_tokens: u64) -> Self {
        Self {
            tokens: max_tokens,
            window_start: Instant::now(),
        }
    }

    fn try_consume(&mut self, cost: u64, max_tokens: u64, window: Duration) -> bool {
        // Check if window has expired
        if self.window_start.elapsed() >= window {
            self.tokens = max_tokens;
            self.window_start = Instant::now();
        }

        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    fn remaining(&self, max_tokens: u64) -> u64 {
        self.tokens.min(max_tokens)
    }

    fn reset_at(&self, window: Duration) -> Instant {
        self.window_start + window
    }
}

/// Rate limit result
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Remaining requests in window
    pub remaining: u64,
    /// Reset time
    pub reset_at: Instant,
    /// Retry after (if denied)
    pub retry_after: Option<Duration>,
}

impl RateLimitResult {
    /// Allowed result
    pub fn allowed(remaining: u64, reset_at: Instant) -> Self {
        Self {
            allowed: true,
            remaining,
            reset_at,
            retry_after: None,
        }
    }

    /// Denied result
    pub fn denied(remaining: u64, reset_at: Instant, retry_after: Duration) -> Self {
        Self {
            allowed: false,
            remaining,
            reset_at,
            retry_after: Some(retry_after),
        }
    }
}

/// Token bucket rate limiter
pub struct RateLimiter {
    config: RateLimitConfig,
    entries: Arc<RwLock<HashMap<RateLimitKey, RateLimitEntry>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with defaults
    pub fn default_limiter() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Check and consume a token
    pub async fn check(&self, key: &RateLimitKey) -> RateLimitResult {
        let mut entries = self.entries.write().await;

        let entry = entries
            .entry(key.clone())
            .or_insert_with(|| RateLimitEntry::new(self.config.max_requests));

        let _remaining = entry.remaining(self.config.max_requests);
        let reset_at = entry.reset_at(self.config.window);

        if entry.try_consume(1, self.config.max_requests, self.config.window) {
            RateLimitResult::allowed(
                entry.remaining(self.config.max_requests),
                entry.reset_at(self.config.window),
            )
        } else {
            let retry_after = Duration::from_secs(1); // Minimum retry interval
            RateLimitResult::denied(0, reset_at, retry_after)
        }
    }

    /// Check without consuming
    pub async fn peek(&self, key: &RateLimitKey) -> RateLimitResult {
        let entries = self.entries.read().await;

        if let Some(entry) = entries.get(key) {
            let remaining = entry.remaining(self.config.max_requests);
            let reset_at = entry.reset_at(self.config.window);
            RateLimitResult::allowed(remaining, reset_at)
        } else {
            RateLimitResult::allowed(
                self.config.max_requests,
                Instant::now() + self.config.window,
            )
        }
    }

    /// Reset rate limit for a key
    pub async fn reset(&self, key: &RateLimitKey) {
        let mut entries = self.entries.write().await;
        entries.remove(key);
    }

    /// Reset all rate limits
    pub async fn reset_all(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    /// Get current entries count
    pub async fn len(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    /// Check if empty
    pub async fn is_empty(&self) -> bool {
        let entries = self.entries.read().await;
        entries.is_empty()
    }

    /// Cleanup expired entries
    pub async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, entry| {
            entry.reset_at(self.config.window) > Instant::now()
                || entry.tokens < self.config.max_requests
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
        });

        let key = RateLimitKey::anonymous(Some("127.0.0.1".to_string()), "test_action");

        // First 3 should be allowed
        for _ in 0..3 {
            let result = limiter.check(&key).await;
            assert!(result.allowed);
        }

        // 4th should be denied
        let result = limiter.check(&key).await;
        assert!(!result.allowed);
        assert!(result.retry_after.is_some());
    }

    #[tokio::test]
    async fn test_different_keys() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
        });

        let key1 = RateLimitKey::anonymous(Some("127.0.0.1".to_string()), "action1");
        let key2 = RateLimitKey::anonymous(Some("127.0.0.2".to_string()), "action1");

        // Each key should have its own limit
        let result1 = limiter.check(&key1).await;
        let result2 = limiter.check(&key2).await;

        assert!(result1.allowed);
        assert!(result2.allowed);
    }
}
