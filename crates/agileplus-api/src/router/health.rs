//! Health check handlers and service probe utilities.
//!
//! Provides:
//! - Simple health check (`/health`)
//! - Detailed health check with service probes (`/detailed-health`)
//! - TCP connection probing for external services (NATS, Dragonfly, Neo4j, MinIO)

use std::time::Instant;

use axum::Json;

use crate::responses::{DetailedHealthResponse, SimpleHealthResponse};
use crate::state::AppState;
use agileplus_domain::ports::vcs::VcsPort;
use agileplus_domain::ports::{ObservabilityPort, StoragePort};

/// `GET /health` — simple health check, no auth required.
pub async fn simple_health_handler() -> Json<SimpleHealthResponse> {
    Json(SimpleHealthResponse::healthy())
}

/// `GET /detailed-health` — aggregated health check, no auth required (T070).
pub async fn health_handler<S, V, O>(
    axum::extract::State(app): axum::extract::State<AppState<S, V, O>>,
) -> Json<DetailedHealthResponse>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    use std::collections::HashMap;

    // Probe storage with a lightweight call.
    let mut services: HashMap<String, crate::responses::ServiceHealth> = HashMap::new();

    let t0 = Instant::now();
    let sqlite_health = match app.storage.list_all_features().await {
        Ok(_) => crate::responses::ServiceHealth::healthy(t0.elapsed().as_millis() as u64),
        Err(e) => crate::responses::ServiceHealth::unavailable(e.to_string()),
    };
    services.insert("sqlite".to_owned(), sqlite_health);

    // --- Env-gated service probes (2 s timeout each) ---
    let probe_timeout = std::time::Duration::from_secs(2);

    // NATS — check NATS_URL, attempt TCP connect
    services.insert(
        "nats".to_owned(),
        probe_tcp_env("NATS_URL", probe_timeout).await,
    );

    // Dragonfly / Redis — check DRAGONFLY_URL then REDIS_URL
    services.insert(
        "dragonfly".to_owned(),
        probe_tcp_env_multi(&["DRAGONFLY_URL", "REDIS_URL"], probe_timeout).await,
    );

    // Neo4j — check NEO4J_URI, attempt TCP connect to host:port
    services.insert(
        "neo4j".to_owned(),
        probe_tcp_env("NEO4J_URI", probe_timeout).await,
    );

    // MinIO/S3 — check S3_ENDPOINT, attempt TCP connect
    services.insert(
        "minio".to_owned(),
        probe_tcp_env("S3_ENDPOINT", probe_timeout).await,
    );

    let overall = DetailedHealthResponse::compute_status(&services).to_string();

    Json(DetailedHealthResponse {
        status: overall,
        timestamp: chrono::Utc::now().to_rfc3339(),
        services,
        api: crate::responses::ApiHealth {
            status: "healthy".to_owned(),
            uptime_seconds: 0, // uptime tracking requires a startup timestamp in AppState
        },
    })
}

/// Probe a single env var: if set, TCP-connect to host:port with timeout.
/// Returns `not_configured` if the env var is absent.
async fn probe_tcp_env(
    env_key: &str,
    timeout: std::time::Duration,
) -> crate::responses::ServiceHealth {
    let url = match std::env::var(env_key) {
        Ok(v) => v,
        Err(_) => return crate::responses::ServiceHealth::not_configured(),
    };
    probe_tcp_url(&url, timeout).await
}

/// Try multiple env var names in order; return the first that is set and probed.
/// If none are set, return `not_configured`.
async fn probe_tcp_env_multi(
    env_keys: &[&str],
    timeout: std::time::Duration,
) -> crate::responses::ServiceHealth {
    for key in env_keys {
        if let Ok(url) = std::env::var(key) {
            return probe_tcp_url(&url, timeout).await;
        }
    }
    crate::responses::ServiceHealth::not_configured()
}

/// Parse `host:port` from a URL string and TCP-connect with the given timeout.
/// Accepts schemes like `http://host:port`, `nats://host:port`, or bare `host:port`.
async fn probe_tcp_url(url: &str, timeout: std::time::Duration) -> crate::responses::ServiceHealth {
    let addr = extract_host_port(url);
    let t0 = Instant::now();
    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => crate::responses::ServiceHealth::healthy(t0.elapsed().as_millis() as u64),
        Ok(Err(e)) => crate::responses::ServiceHealth::unavailable(format!("{addr}: {e}")),
        Err(_) => {
            crate::responses::ServiceHealth::unavailable(format!("{addr}: connection timed out"))
        }
    }
}

/// Extract `host:port` from a URL or bare address string.
fn extract_host_port(url: &str) -> String {
    // Strip scheme prefix (e.g. "nats://", "http://")
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("nats://"))
        .or_else(|| url.strip_prefix("bolt://"))
        .or_else(|| url.strip_prefix("bolt+routing://"))
        .unwrap_or(url);
    // Strip trailing path/query
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    // If port is missing, default to 4222 for nats-like schemes, 80 otherwise
    if host_port.contains(':') {
        host_port.to_string()
    } else if url.starts_with("nats://") {
        format!("{host_port}:4222")
    } else {
        format!("{host_port}:80")
    }
}
