//! HuggingFace `SourceAdapter` implementation.
//!
//! Walks the public HF Hub API:
//! - [`GET /api/models?search={q}&limit={n}`] for `list_candidates`
//! - [`GET /api/models/{id}`] for metadata
//! - [`GET /api/models/{id}/tree/main`] for top-level file listing
//! - [`GET /api/models/{id}/raw/main/README.md`] for the model card
//!
//! All four endpoints are public; authentication only widens rate limits
//! and unlocks gated models. Tokens are read from `HF_TOKEN`.
//!
//! The trait surface is synchronous, but the underlying [`reqwest::Client`]
//! is async. To bridge the gap we lazily build a private
//! [`tokio::runtime::Runtime`] on first sync call; lazy populate and
//! any other async code paths use the async helpers directly and never
//! touch the inner runtime.

use std::sync::{Arc, OnceLock};

use hwledger_search_core::{CandidateId, CoreError, RawModel, SourceAdapter};

use crate::error::IngestError;
use crate::rate_limit::{RateLimitPolicy, parse_retry_after};

/// Default HF Hub base URL.
const DEFAULT_HUB_URL: &str = "https://huggingface.co";

/// Environment variable for the HF Hub base URL (mostly used by CI).
const ENV_HUB_URL: &str = "HF_HUB_URL";
/// Environment variable for the HF token.
const ENV_HF_TOKEN: &str = "HF_TOKEN";

/// Current unix timestamp in seconds. Wrapped so callers in async paths
/// don't have to repeat the `SystemTime::now()` dance and so test stubs
/// can override it later if needed.
fn now_ms_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Stable adapter name. Used as the `source` field on every
/// [`CandidateId`] we emit.
const ADAPTER_NAME: &str = "huggingface";

/// Concrete [`SourceAdapter`] for the HuggingFace Hub.
#[derive(Debug)]
pub struct HuggingFaceAdapter {
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
    runtime: Arc<OnceLock<tokio::runtime::Runtime>>,
    /// Per-request rate-limit policy. When `Some`, every outbound HTTP call
    /// is paced by `min_interval_ms` and 429s are retried up to `max_retries`.
    policy: Option<RateLimitPolicy>,
    /// Timestamp (unix seconds) of the last completed HTTP request.
    last_request: std::sync::Mutex<u64>,
}

impl HuggingFaceAdapter {
    /// Build a new adapter pinned to the default HF Hub URL with no
    /// authentication token. Use [`HuggingFaceAdapter::with_token`] to
    /// attach credentials.
    pub fn new() -> Self {
        // Bound outbound HTTP waits so an unreachable host fails fast
        // instead of hanging the test suite / CLI for minutes. Without
        // these, list_candidates against `http://127.0.0.1:1` or any
        // partitioned network will block on the OS connect timeout
        // (~tens of seconds) per request, multiplied by every retry
        // and every candidate.
        Self {
            base_url: DEFAULT_HUB_URL.to_string(),
            token: None,
            client: reqwest::Client::builder()
                .user_agent(concat!("hwledger-search-ingest/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client builder with default config"),
            runtime: Arc::new(OnceLock::new()),
            policy: None,
            last_request: std::sync::Mutex::new(0),
        }
    }

    /// Builder-style setter that overrides the auth token.
    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }

    /// Builder-style setter that configures rate limiting on outbound requests.
    pub fn with_rate_limit(mut self, policy: RateLimitPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Borrow the configured HF token, if any. Returns `None` when the
    /// adapter was constructed without authentication.
    pub fn token_snapshot(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Build a new adapter from `HF_HUB_URL` and `HF_TOKEN` environment
    /// variables. Missing `HF_TOKEN` is not an error — it just yields an
    /// unauthenticated adapter — but a non-UTF-8 value is.
    pub fn from_env() -> Result<Self, IngestError> {
        let base_url = std::env::var(ENV_HUB_URL).unwrap_or_else(|_| DEFAULT_HUB_URL.to_string());
        let token = std::env::var(ENV_HF_TOKEN).ok().filter(|t| !t.is_empty());
        let mut a = Self::new();
        a.base_url = base_url;
        a.token = token;
        Ok(a)
    }

    /// Build the request URL for a model-metadata lookup.
    fn model_url(&self, id: &str) -> String {
        format!("{}/api/models/{}", self.base_url.trim_end_matches('/'), id)
    }

    /// Build the request URL for a model's tree listing.
    fn tree_url(&self, id: &str) -> String {
        format!("{}/api/models/{}/tree/main", self.base_url.trim_end_matches('/'), id)
    }

    /// Build the request URL for a model's raw README card.
    fn card_url(&self, id: &str) -> String {
        // The HF convention is "{id}/raw/main/README.md" — relative to
        // the resolver base URL, not the API.
        format!("{}/{}/raw/main/README.md", self.base_url.trim_end_matches('/'), id)
    }

    /// Attach the auth token (if any) to a request builder.
    fn authed(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.token {
            Some(t) => req.bearer_auth(t),
            None => req,
        }
    }

    /// Rate-limited send with 429-retry-on-backoff. Respects `policy`
    /// for inter-request pacing and Retry-After header on 429.
    async fn send_with_retry(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response, IngestError> {
        let policy = self.policy.unwrap_or(RateLimitPolicy::default());
        let mut retries = 0u32;
        loop {
            // Enforce minimum interval between requests. `std::sync::Mutex`
            // is not re-entrant — we must drop the guard before the
            // `await`, and re-acquire after sleeping — and we must not
            // hold it across the unconditional final stamp either, or
            // the first call (where `last_request` is 0 and
            // `wait_ms` saturates to 0) self-deadlocks.
            let now_ms: u64 = {
                let last = self.last_request.lock().expect("lock poisoned");
                let elapsed_ms = (now_ms_unix() * 1000).saturating_sub(*last * 1000);
                let wait_ms = policy.min_interval_ms.saturating_sub(elapsed_ms);
                if wait_ms > 0 {
                    drop(last);
                    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                }
                // Capture the timestamp for the post-sleep stamp; the
                // post-sleep stamp below re-locks but only after we've
                // dropped the guard above.
                now_ms_unix()
            };
            *self.last_request.lock().expect("lock poisoned") = now_ms;

            let resp = req.try_clone().expect("request must be cloneable")
                .send()
                .await
                .map_err(|e| IngestError::http(e.to_string()))?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                retries += 1;
                if retries > policy.max_retries {
                    return Err(IngestError::RateLimited);
                }
                // Parse Retry-After header for adaptive backoff.
                let retry_secs = resp.headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| parse_retry_after(v, {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    }))
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(policy.min_interval_ms);
                tracing::warn!(retry_secs, retries, "huggingface: 429 rate-limited, backing off");
                tokio::time::sleep(std::time::Duration::from_millis(retry_secs)).await;
                continue;
            }

            return Ok(resp);
        }
    }

    /// Translate a transport failure into a typed [`IngestError`].
    fn classify(status: reqwest::StatusCode) -> IngestError {
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            IngestError::RateLimited
        } else if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            IngestError::AuthRequired
        } else if status == reqwest::StatusCode::NOT_FOUND {
            IngestError::http("upstream returned 404 not found")
        } else {
            IngestError::http(format!("upstream returned {status}"))
        }
    }

    /// Async metadata fetch — used by both the sync trait methods and the
    /// async lazy-populate path.
    pub(crate) async fn fetch_raw_async(&self, id: &CandidateId) -> Result<RawModel, IngestError> {
        let mut raw = RawModel::new(id.id.as_str(), ADAPTER_NAME);

        // 1. Metadata blob.
        let meta_url = self.model_url(&id.id);
        let resp = self.send_with_retry(self.authed(self.client.get(&meta_url))).await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::classify(status));
        }
        let meta: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| IngestError::http(e.to_string()))?;

        raw.downloads = meta.get("downloads").and_then(serde_json::Value::as_u64);
        raw.likes = meta.get("likes").and_then(serde_json::Value::as_u64);
        raw.last_modified = meta
            .get("lastModified")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        raw.pipeline_tag = meta
            .get("pipeline_tag")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        // The `/api/models/{id}` payload embeds a `config` key holding
        // the raw `config.json` blob for many models. Fall back to
        // fetching the dedicated `config.json` if it's missing.
        if let Some(cfg) = meta.get("config").cloned() {
            raw.config_json = Some(cfg);
        } else if let Ok(cfg) = self.fetch_config_json_async(&id.id).await {
            raw.config_json = Some(cfg);
        }

        // 2. Tree listing.
        let tree_url = self.tree_url(&id.id);
        let resp = self.send_with_retry(self.authed(self.client.get(&tree_url))).await?;
        if resp.status().is_success() {
            let tree: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| IngestError::http(e.to_string()))?;
            raw.tree_entries = crate::tree_parser::parse_tree_value(&tree)
                .into_iter()
                .map(|e| e.path)
                .collect();
        }

        // 3. README card — best-effort.
        if let Ok(text) = self.fetch_card_async(&id.id).await {
            raw.card_text = Some(text);
        }

        Ok(raw)
    }

    /// Fetch the raw `config.json` blob as a [`serde_json::Value`].
    async fn fetch_config_json_async(&self, id: &str) -> Result<serde_json::Value, IngestError> {
        let url = format!(
            "{}/{}/raw/main/config.json",
            self.base_url.trim_end_matches('/'),
            id
        );
        let resp = self.send_with_retry(self.authed(self.client.get(&url))).await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::classify(status));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| IngestError::http(e.to_string()))
    }

    /// Fetch the raw README card text.
    async fn fetch_card_async(&self, id: &str) -> Result<String, IngestError> {
        let url = self.card_url(id);
        let resp = self.send_with_retry(self.authed(self.client.get(&url))).await?;
        if !resp.status().is_success() {
            return Err(IngestError::http(format!(
                "card fetch returned {}",
                resp.status()
            )));
        }
        resp.text()
            .await
            .map_err(|e| IngestError::http(e.to_string()))
    }

    /// Drive an async future on a private runtime — used by the sync
    /// trait surface. The runtime is built lazily on first call.
    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        // Fast path: if we're already inside a tokio runtime, drive
        // the future inline. `block_in_place` lets the worker thread
        // block without stalling the whole reactor.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(fut))
        } else {
            let rt = self
                .runtime
                .get_or_init(|| tokio::runtime::Runtime::new().expect("tokio runtime"))
                .handle()
                .clone();
            rt.block_on(fut)
        }
    }
}

impl Default for HuggingFaceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceAdapter for HuggingFaceAdapter {
    fn name(&self) -> &str {
        ADAPTER_NAME
    }

    fn list_candidates(&self, query: Option<&str>, limit: usize) -> Vec<CandidateId> {
        if limit == 0 {
            return Vec::new();
        }
        let url = match query {
            Some(q) if !q.is_empty() => format!(
                "{}/api/models?search={}&limit={}",
                self.base_url.trim_end_matches('/'),
                urlencode(q),
                limit
            ),
            _ => format!(
                "{}/api/models?limit={}",
                self.base_url.trim_end_matches('/'),
                limit
            ),
        };
        let resp = match self.block_on(async { self.send_with_retry(self.authed(self.client.get(&url))).await }) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "huggingface list_candidates: send failed");
                return Vec::new();
            }
        };
        let status = resp.status();
        if !status.is_success() {
            tracing::warn!(%status, "huggingface list_candidates: non-success");
            return Vec::new();
        }
        let body: serde_json::Value = match self.block_on(resp.json()) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "huggingface list_candidates: json failed");
                return Vec::new();
            }
        };
        let Some(arr) = body.as_array() else {
            return Vec::new();
        };
        arr.iter()
            .take(limit)
            .filter_map(|item| {
                let id = item
                    .get("modelId")
                    .or_else(|| item.get("id"))
                    .and_then(serde_json::Value::as_str)?;
                Some(CandidateId::new(ADAPTER_NAME, id))
            })
            .collect()
    }

    fn fetch_raw(&self, id: &CandidateId) -> Result<RawModel, CoreError> {
        let raw = self
            .block_on(self.fetch_raw_async(id))
            .map_err(IngestError::into_core)?;
        Ok(raw)
    }
}

/// Minimal, dependency-free URL encoder for the `search` query param.
/// Only handles the percent-unsafe runes we expect to encounter in
/// model names; anything else (including valid UTF-8) gets passed
/// through verbatim.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_passes_alphanumerics() {
        assert_eq!(urlencode("qwen2.5-7b-instruct"), "qwen2.5-7b-instruct");
    }

    #[test]
    fn urlencode_escapes_unsafe() {
        assert_eq!(urlencode("foo bar"), "foo%20bar");
        assert_eq!(urlencode("a&b"), "a%26b");
    }

    #[test]
    fn adapter_name_is_stable() {
        let a = HuggingFaceAdapter::new();
        assert_eq!(a.name(), "huggingface");
        assert_eq!(a.base_url, DEFAULT_HUB_URL);
        assert!(a.token.is_none());
    }

    #[test]
    fn with_token_attaches_bearer() {
        let a = HuggingFaceAdapter::new().with_token("secret".to_string());
        assert_eq!(a.token.as_deref(), Some("secret"));
    }
}
