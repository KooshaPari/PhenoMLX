//! Seed bridge for the dashboard.
//!
//! Two modes:
//! - **Standalone** (default): builds a hardcoded `DashboardStore` from `DashboardStore::seeded()`.
//!   Used by `cargo test`, CI, and any run where the `agileplus-api` daemon isn't running.
//! - **Live**: when called from `main.rs`, we try `reqwest` against `http://127.0.0.1:3000`
//!   and replace the hardcoded projects/modules/cycles with the api's responses.
//!   If anything fails (no server, timeout, parse error), we fall back to the seed unchanged.
//!
//! The api endpoints used:
//! - `GET /api/modules`   — public, returns `Vec<ModuleResponse>`
//! - `GET /api/cycles`    — public, returns `Vec<CycleResponse>`
//! - `GET /api/v1/projects` — protected, needs `AGILEPLUS_API_KEY`. If the key isn't set,
//!   the seed keeps the default project list; if the call fails, we ignore the error
//!   (graceful degradation — the dashboard already shows the seed fallback banner).
//!
//! Each call has a 3-second timeout so a stalled api never blocks dashboard startup.

use std::time::Duration;

use agileplus_domain::domain::cycle::Cycle;
use agileplus_domain::domain::module::Module;
use agileplus_domain::domain::project::Project;

use crate::app_state::DashboardStore;

const API_BASE_DEFAULT: &str = "http://127.0.0.1:3000";
const API_TIMEOUT_SECS: u64 = 3;

/// Try to fetch the live project/module/cycle lists from `agileplus-api` and merge them
/// into `store`. On any failure (no server, timeout, parse error, missing fields),
/// `store` is returned unchanged — the hardcoded seed is the fallback.
pub async fn try_merge_from_api(mut store: DashboardStore) -> DashboardStore {
    let base = std::env::var("AGILEPLUS_API_BASE").unwrap_or_else(|_| API_BASE_DEFAULT.to_string());

    // Public endpoints first — no auth required.
    if let Some(modules) = fetch_json::<Vec<Module>>(&format!("{}/api/modules", base)).await
        && !modules.is_empty() {
            store.modules = modules;
        }
    if let Some(cycles) = fetch_json::<Vec<Cycle>>(&format!("{}/api/cycles", base)).await
        && !cycles.is_empty() {
            // Rebuild the cycle -> features index from whatever features are in the store.
            store.cycles = cycles;
            store.cycle_features.clear();
            for cycle in &store.cycles {
                // features in this cycle are those matching the cycle_id (best effort).
                let fids: Vec<i64> = store
                    .features
                    .iter()
                    .filter(|f| f.module_id == Some(cycle.id))
                    .map(|f| f.id)
                    .collect();
                store.cycle_features.insert(cycle.id, fids);
            }
        }

    // Protected — needs AGILEPLUS_API_KEY.
    if let Ok(api_key) = std::env::var("AGILEPLUS_API_KEY")
        && !api_key.is_empty()
            && let Some(projects) = fetch_json_with_header::<Vec<Project>>(
                &format!("{}/api/v1/projects", base),
                &api_key,
            )
            .await
                && !projects.is_empty() {
                    // If the api returned projects, prefer them over the hardcoded seed.
                    // Active project stays at the seed's default (Some(1)) so dashboard pages
                    // don't break when api and seed IDs diverge.
                    if let Some(first) = projects.first().cloned() {
                        store.projects = projects;
                        if store.active_project_id.is_none() {
                            store.active_project_id = Some(first.id);
                        }
                    }
                }

    store
}

async fn fetch_json<T: serde::de::DeserializeOwned + Send>(url: &str) -> Option<T> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(API_TIMEOUT_SECS))
        .build()
        .ok()?;
    client.get(url).send().await.ok()?.json::<T>().await.ok()
}

async fn fetch_json_with_header<T: serde::de::DeserializeOwned + Send>(
    url: &str,
    api_key: &str,
) -> Option<T> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(API_TIMEOUT_SECS))
        .build()
        .ok()?;
    client
        .get(url)
        .header("X-API-Key", api_key)
        .send()
        .await
        .ok()?
        .json::<T>()
        .await
        .ok()
}

/// Build the seed `DashboardStore` (hardcoded data).
///
/// Used as the default state for the dashboard, plus the fallback when the api is
/// unreachable.
pub fn build_dashboard_store() -> DashboardStore {
    DashboardStore::seeded()
}
