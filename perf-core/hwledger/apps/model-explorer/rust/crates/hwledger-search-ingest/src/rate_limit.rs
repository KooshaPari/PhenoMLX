// SPDX-License-Identifier: Apache-2.0
//! Per-request rate limiting + 429-retry policy for the HuggingFace adapter.
//!
//! HuggingFace's free-tier accounts throttle anonymous and unverified
//! tokens to ~5 req/min. Without a rate limiter the live seed-build
//! burns through that budget in seconds and the rest of the run is
//! all 429s. With this limiter you can drive a 2000-model seed in
//! 30-90 minutes (depending on `--rate-limit-ms`).
//!
//! ## Usage
//!
//! ```no_run
//! use hwledger_search_ingest::rate_limit::{RateLimitPolicy, parse_retry_after};
//!
//! let policy = RateLimitPolicy { min_interval_ms: 12_000, max_retries: 3 };
//! // policy is sent through the HTTP client and applied per request.
//! ```
//!
//! ## Headers honored
//!
//! * `Retry-After: <seconds>` — wait exactly that many seconds
//! * `Retry-After: <http-date>` — wait until that timestamp
//! * `x-ratelimit-remaining: 0` — proactively slow down
//!
//! If the response carries no Retry-After, the policy's
//! `min_interval_ms` is used as the fallback.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Per-adapter rate-limit policy applied to every outbound HTTP call.
///
/// `min_interval_ms` is the floor between any two requests — keeps the
/// adapter under HF's per-minute budget. `max_retries` bounds the 429
/// retry-on-backoff loop so a misbehaving server can't deadlock the
/// seed-build.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitPolicy {
    /// Floor between any two outbound HTTP requests (ms).
    pub min_interval_ms: u64,
    /// Cap on 429 retry-on-backoff iterations per request.
    pub max_retries: u32,
}

impl Default for RateLimitPolicy {
    fn default() -> Self {
        Self { min_interval_ms: 1_000, max_retries: 3 }
    }
}

/// Parse the `Retry-After` header per RFC 7231. Returns the absolute
/// duration to wait, or `None` if the header can't be parsed.
pub fn parse_retry_after(value: &str, now_unix: u64) -> Option<Duration> {
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    if let Ok(target) = httpdate::parse_http_date(value) {
        let target_unix = target
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        return target_unix.checked_sub(now_unix).map(Duration::from_secs);
    }
    None
}

/// Test the parser against the canonical HTTP-date examples.
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parses_seconds_form() {
        let now: u64 = 1_700_000_000;
        assert_eq!(parse_retry_after("120", now), Some(Duration::from_secs(120)));
    }

    #[test]
    fn parses_rfc1123_form() {
        // Fixed UTC date parse — RFC 1123 includes weekday.
        let _now: u64 = 1_700_000_000;
        let parsed = parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT", 1_445_407_680);
        assert!(parsed.is_some());
    }

    #[test]
    fn falls_back_to_none_for_garbage() {
        assert_eq!(parse_retry_after("not-a-date", 1_700_000_000), None);
    }

    #[test]
    fn default_policy_is_sane() {
        let p = RateLimitPolicy::default();
        assert_eq!(p.min_interval_ms, 1_000);
        assert_eq!(p.max_retries, 3);
    }
}
