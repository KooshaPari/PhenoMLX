//! Axum route handlers for the dashboard. (T077)
//!
//! This module provides HTTP request handlers for the AgilePlus dashboard,
//! organized into per-domain submodules:
//! - **pages**: Root, home, features, events, settings, hub pages
//! - **dashboard**: Kanban board, work packages, SSE streams
//! - **features**: Feature detail, transitions, events, media
//! - **evidence**: Evidence bundles, artifacts, generation
//! - **agents**: Agent activity detection
//! - **health**: Service health monitoring and management
//! - **settings**: Configuration pages and persistence
//! - **helpers**: Shared utility functions and types
//!
//! Pattern: if the request carries `HX-Request: true`, return only the
//! relevant partial; otherwise return the full page layout.

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::app_state::SharedState;

pub mod agents;
pub mod dashboard;
pub mod evidence;
pub mod features;
pub mod health;
pub mod helpers;
pub mod pages;
pub mod settings;

// ── Route Handler Re-exports ──────────────────────────────────────────────
// Exported for backward compatibility with call sites like routes::feature_detail

// From pages
pub use pages::{dashboard_page, events_page, features_page, home, hub_page, root, settings_page};

// From dashboard
pub use dashboard::{
    WorkPackageJson, all_work_packages_json, epics_stories_json, kanban_board, project_switcher,
    sse_stream, switch_project, time_footer, wp_list,
};

// From features
pub use features::{
    FeatureTransitionForm, feature_detail, feature_events, feature_media, feature_page,
    feature_transition,
};

// From evidence
pub use evidence::{
    EvidenceArtifactJson, EvidenceGalleryJson, evidence_content, evidence_preview,
    feature_evidence_generate, feature_evidence_json, feature_evidence_list,
};

// From agents
pub use agents::{agent_activity, agents_json, test_agent_connection};

// From health
pub use health::{
    HealthStatus, ServiceHealthJson, health_json, health_page, health_panel, patch_service_config,
    restart_service, toggle_service,
};

// From settings
pub use settings::{
    AgentConfig,
    AgentSettingsForm,
    // Types
    Config,
    DashboardConfig,
    DashboardSettingsForm,
    PlaneConfig,
    PlaneSettingsForm,
    ServiceConfig,
    ServiceSettingsForm,
    SingleServiceTestForm,
    agent_settings_page,
    plane_settings_page,
    save_agent_settings,
    save_dashboard_settings,
    save_plane_settings,
    save_services_settings,
    services_settings_page,
    test_plane_connection,
    test_service_connection,
};

// ── Event Timeline Handler ─────────────────────────────────────────────

pub async fn event_timeline(
    axum::extract::State(_state): axum::extract::State<SharedState>,
) -> axum::response::Response {
    use crate::templates::EventTimelinePartial;
    helpers::render(EventTimelinePartial {
        feature_id: 0,
        events: vec![],
    })
}

// ── Router builder ───────────────────────────────────────────────────────

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/home", get(home))
        .route("/dashboard", get(dashboard_page))
        .route("/features", get(features_page))
        .route("/features/{id}", get(feature_page))
        .route("/events", get(events_page))
        // NOTE: /health is owned by agileplus-api (router.rs, T070, detailed
        // JSON health). The HTML health page lives at /health-page to avoid
        // a route conflict panic when build_router merges the two routers.
        .route("/health-page", get(health_page))
        .route("/settings", get(settings_page))
        .route("/settings/plane", get(plane_settings_page))
        .route("/settings/agents", get(agent_settings_page))
        .route("/settings/services", get(services_settings_page))
        .route("/api/settings/services", post(save_services_settings))
        .route("/api/settings/services/test", post(test_service_connection))
        .route("/hub", get(hub_page))
        .route("/api/settings/plane", post(save_plane_settings))
        .route("/api/settings/plane/test", post(test_plane_connection))
        .route("/api/settings/agents", post(save_agent_settings))
        .route(
            "/api/settings/agents/test-connection",
            post(test_agent_connection),
        )
        .route("/api/settings/dashboard", post(save_dashboard_settings))
        .route(
            "/api/dashboard/services/{name}/restart",
            post(restart_service),
        )
        .route(
            "/api/dashboard/services/{name}/config",
            axum::routing::patch(patch_service_config),
        )
        .route(
            "/api/dashboard/services/{name}/toggle",
            post(toggle_service),
        )
        .route("/api/dashboard/kanban", get(kanban_board))
        .route("/api/dashboard/features/{id}", get(feature_detail))
        .route("/api/dashboard/features/{id}/work-packages", get(wp_list))
        .route("/api/dashboard/features/{id}/events", get(feature_events))
        .route("/api/dashboard/features/{id}/media", get(feature_media))
        // HTML partial endpoints (HTMX-compatible)
        .route("/api/dashboard/health", get(health_panel))
        .route("/api/dashboard/events", get(event_timeline))
        .route("/api/dashboard/agents", get(agent_activity))
        // JSON API endpoints (for polling from JavaScript templates)
        .route("/api/dashboard/agents.json", get(agents_json))
        .route("/api/dashboard/health.json", get(health_json))
        .route(
            "/api/dashboard/work-packages.json",
            get(all_work_packages_json),
        )
        .route("/api/dashboard/epics-stories.json", get(epics_stories_json))
        .route("/api/dashboard/projects", get(project_switcher))
        .route(
            "/api/dashboard/projects/{id}/activate",
            post(switch_project),
        )
        .route("/api/time", get(time_footer))
        .route("/api/stream", get(sse_stream))
        // NOTE: /api/v1/stream is owned by agileplus-api (router.rs, T069).
        // Previously this crate registered an alias (#334), but that caused a
        // route conflict panic when build_router merged the two routers.
        .route("/api/stream-placeholder", get(sse_stream))
        .route(
            "/api/evidence/{feature_id}/{artifact_id}/content",
            get(evidence_content),
        )
        .route(
            "/api/evidence/{feature_id}/{artifact_id}/preview",
            get(evidence_preview),
        )
        .route("/api/features/{id}/evidence", get(feature_evidence_list))
        .route(
            "/api/features/{id}/evidence/generate",
            post(feature_evidence_generate),
        )
        .route(
            "/api/dashboard/features/{id}/evidence.json",
            get(feature_evidence_json),
        )
        .route("/api/features/{id}/transition", post(feature_transition))
        .route("/api/dashboard/governance/status", get(governance_status))
        .route("/api/dashboard/plane/sync", get(plane_sync_status))
        .route(
            "/api/dashboard/plane/daemon/start",
            post(plane_daemon_start),
        )
        .route("/api/dashboard/plane/daemon/stop", post(plane_daemon_stop))
        .route(
            "/api/dashboard/plane/daemon/status",
            get(plane_daemon_status),
        )
        .with_state(state)
}

// ============================================================================
// LIVE STATUS HANDLERS — read from real clients in DashboardStore state
// ============================================================================

/// GET /api/dashboard/governance/status — live audit stats from GovernanceClient
async fn governance_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let guard = state.read().await;
    match guard.governance_client.as_ref() {
        Some(client) => {
            // status() returns GovernanceStatus directly (not Result)
            let status = client.status().await;
            Json(serde_json::json!({
                "available": true,
                "initialized": status.initialized,
                "connection_status": format!("{:?}", status.connection_status),
                "remote_enabled": status.remote_enabled,
                "local_enabled": status.local_enabled,
                "sync_enabled": status.sync_enabled,
                "last_sync": status.last_sync,
                "pending_operations": status.pending_operations,
                "audits_total": status.stats.total,
                "audits_today": status.stats.today,
                "audit_errors": status.stats.errors,
            }))
        }
        None => Json(serde_json::json!({"available": false, "reason": "not_initialized"})),
    }
}

/// GET /api/dashboard/plane/sync — live work-item count from PlaneClient
async fn plane_sync_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let guard = state.read().await;
    match guard.plane_client.as_ref() {
        Some(client) => {
            // list_work_items returns anyhow::Result<Vec<PlaneWorkItemResponse>>
            let work_items = client.list_work_items().await;
            let synced_at = chrono::Utc::now().to_rfc3339();
            match work_items {
                Ok(items) => Json(serde_json::json!({
                    "available": true,
                    "work_items": items.len(),
                    "synced_at": synced_at,
                })),
                Err(e) => Json(serde_json::json!({
                    "available": true,
                    "error": format!("{}", e),
                    "synced_at": synced_at,
                })),
            }
        }
        None => Json(serde_json::json!({"available": false, "reason": "not_initialized"})),
    }
}

// ============================================================================
// PLANE DAEMON CONTROL — start/stop/status for the PlaneSyncDaemon
// ============================================================================

/// POST /api/dashboard/plane/daemon/start — start the Plane sync daemon
async fn plane_daemon_start(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let guard = state.write().await;
    match guard.plane_daemon.as_ref() {
        Some(daemon) => {
            daemon.start().await;
            Json(serde_json::json!({
                "started": true,
                "already_running": true,
            }))
        }
        None => Json(serde_json::json!({
            "started": false,
            "reason": "not_initialized",
        })),
    }
}

/// POST /api/dashboard/plane/daemon/stop — stop the Plane sync daemon
async fn plane_daemon_stop(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let guard = state.write().await;
    match guard.plane_daemon.as_ref() {
        Some(daemon) => {
            daemon.stop().await;
            Json(serde_json::json!({"stopped": true}))
        }
        None => Json(serde_json::json!({
            "stopped": false,
            "reason": "not_initialized",
        })),
    }
}

/// GET /api/dashboard/plane/daemon/status — daemon state snapshot
async fn plane_daemon_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let guard = state.read().await;
    match guard.plane_daemon.as_ref() {
        Some(daemon) => {
            let cfg = daemon.config();
            Json(serde_json::json!({
                "available": true,
                "interval_secs": cfg.interval,
                "batch_size": cfg.batch_size,
                "dry_run": cfg.dry_run,
            }))
        }
        None => Json(serde_json::json!({"available": false, "reason": "not_initialized"})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{DashboardStore, default_health};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tower::util::ServiceExt;

    fn make_state() -> SharedState {
        let store = DashboardStore {
            health: default_health(),
            ..Default::default()
        };
        Arc::new(RwLock::new(store))
    }

    #[tokio::test]
    async fn toggle_service_updates_store_and_responds() {
        let state = make_state();
        let app = router(state.clone());

        let request_body = serde_json::json!({ "enabled": false }).to_string();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dashboard/services/NATS/toggle")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(request_body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_text.contains("\"status\":\"ok\""));
        assert!(body_text.contains("\"enabled\":false"));

        let store = state.read().await;
        let health = store.health.iter().find(|s| s.name == "NATS").unwrap();
        assert!(!health.healthy);
        assert!(health.degraded);
    }

    #[tokio::test]
    async fn restart_service_executes_command() {
        let state = make_state();
        let app = router(state.clone());

        unsafe {
            std::env::set_var("AGILEPLUS_SERVICE_RESTART_CMD", "echo restarted {}");
        }

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/dashboard/services/NATS/restart")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_text.contains("\"status\":\"ok\""));
        assert!(body_text.contains("\"service\":\"NATS\""));
        assert!(body_text.contains("restarted NATS"));
    }

    #[test]
    fn test_html_escape_ampersand() {
        assert_eq!(helpers::html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_html_escape_angle_brackets() {
        assert_eq!(helpers::html_escape("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn test_html_escape_quotes() {
        assert_eq!(
            helpers::html_escape("say \"hello\""),
            "say &quot;hello&quot;"
        );
        assert_eq!(helpers::html_escape("it's"), "it&#39;s");
    }

    #[test]
    fn test_is_htmx_true() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("HX-Request", "true".parse().unwrap());
        assert!(helpers::is_htmx(&headers));
    }

    #[test]
    fn test_is_htmx_false_absent() {
        let headers = axum::http::HeaderMap::new();
        assert!(!helpers::is_htmx(&headers));
    }

    #[test]
    fn test_is_htmx_false_wrong_value() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("HX-Request", "1".parse().unwrap());
        assert!(!helpers::is_htmx(&headers));
    }

    #[tokio::test]
    async fn json_endpoints_integrated_agents_in_router() {
        let state = make_state();
        let app = router(state);

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/dashboard/agents.json")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

        let json: serde_json::Value = serde_json::from_str(&body_text).expect("valid JSON");
        assert!(json.get("agents").is_some());
        assert!(json.get("count").is_some());
        assert!(json.get("timestamp").is_some());
    }

    #[tokio::test]
    async fn json_endpoints_integrated_health_in_router() {
        let state = make_state();
        let app = router(state);

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/dashboard/health.json")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

        let json: serde_json::Value = serde_json::from_str(&body_text).expect("valid JSON");
        assert!(json.get("services").is_some());
        assert!(json.get("timestamp").is_some());
        assert!(json.get("all_healthy").is_some());
    }
}
