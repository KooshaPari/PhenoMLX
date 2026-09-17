//! Dashboard route handlers for kanban, work packages, and server-sent events.
//!
//! This module implements the core dashboard UI handlers:
//! - Kanban board view with state-based card grouping and filtering
//! - Work package list views
//! - Project switcher for multi-project filtering
//! - SSE (Server-Sent Events) stream for real-time health/feature updates
//! - JSON APIs for work-packages and epics/stories (used by React dashboard at port 5176)
//!
//! Pattern: if the request carries `HX-Request: true`, return only the relevant
//! partial template; otherwise return the full page layout.

use std::collections::HashMap;
use std::path::PathBuf;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::app_state::SharedState;
use crate::templates::{
    AgentActivityPartial, AgentView, DashboardPage, EventTimelinePartial, EvidenceBundleView,
    FeatureDetailPage, FeatureView, HealthPanelPartial, KanbanPartial, MediaAssetView,
    ProjectSwitcherPartial, ProjectView, ReportArtifactView, WpListPartial, WpView,
};

use super::helpers::{
    DashboardFilter, build_kanban_cards, dashboard_filter_from_query, is_htmx, load_projects,
    render,
};

// â”€â”€ JSON API Response Types â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// JSON response for GET /api/dashboard/work-packages.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkPackageJson {
    pub id: String,
    pub feature_id: i64,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
}

#[allow(dead_code)] // planned dashboard helper - will be wired in upcoming UI integration
fn build_feature_events(
    feature: &FeatureView,
    workpackages: &[WpView],
) -> Vec<crate::templates::EventView> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let mut events = vec![super::helpers::event_view(
        format!("evt-feature-{}-created", feature.id),
        "system",
        format!("Feature '{}' opened in dashboard", feature.slug),
        now.clone(),
    )];

    if !workpackages.is_empty() {
        events.push(super::helpers::event_view(
            format!("evt-feature-{}-sync", feature.id),
            "agent_action",
            format!("{} work package entries synced", workpackages.len()),
            now.clone(),
        ));

        for wp in workpackages {
            events.push(super::helpers::event_view(
                format!("evt-feature-{}-wp-{}", feature.id, wp.id),
                "state_change",
                format!("Work-package {} is in state '{}'", wp.title, wp.state),
                now.clone(),
            ));
        }
    } else {
        events.push(super::helpers::event_view(
            format!("evt-feature-{}-no-wp", feature.id),
            "system",
            "No work packages linked yet",
            now.clone(),
        ));
    }

    events
}

#[allow(dead_code)] // planned dashboard helper - will be wired in upcoming UI integration
fn build_feature_evidence_bundles(
    feature: &FeatureView,
    workpackages: &[WpView],
) -> Vec<EvidenceBundleView> {
    let mut bundles = vec![EvidenceBundleView {
        id: format!("bundle-{id}-summary", id = feature.id),
        fr_id: format!("FR-{id}", id = feature.id),
        evidence_type: "feature_summary".into(),
        wp_id: "dashboard".into(),
        wp_title: feature.title.clone(),
        artifact_path: format!("/artifacts/features/{}.md", feature.slug),
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        artifact_ext: "md".into(),
        status: "available".into(),
        content_preview: Some("# Feature Summary\n\nThis feature provides...".to_string()),
        is_text_artifact: true,
        is_image_artifact: false,
        download_url: format!("/api/evidence/{}/summary/content", feature.id),
        test_passed: None,
        tests_passed_count: 0,
        tests_failed_count: 0,
        test_summary: None,
        commit_count: 0,
        pr_count: 0,
        ci_links: vec![],
        git_commits: vec![],
        pr_links: vec![],
    }];

    for wp in workpackages {
        bundles.push(EvidenceBundleView {
            id: format!("bundle-{fid}-wp-{wid}", fid = feature.id, wid = wp.id),
            fr_id: format!("FR-{fid}", fid = feature.id),
            evidence_type: "workpackage_artifact".into(),
            wp_id: wp.id.to_string(),
            wp_title: wp.title.clone(),
            artifact_path: format!(
                "/artifacts/wp/{wid}/{slug}.json",
                wid = wp.id,
                slug = feature.slug
            ),
            created_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            artifact_ext: "json".into(),
            status: if wp.progress > 0 {
                "accepted"
            } else {
                "generated"
            }
            .into(),
            content_preview: Some(r#"{"status":"generated","progress":0}"#.to_string()),
            is_text_artifact: true,
            is_image_artifact: false,
            download_url: format!("/api/evidence/{}/{}/content", feature.id, wp.id),
            test_passed: None,
            tests_passed_count: 0,
            tests_failed_count: 0,
            test_summary: None,
            commit_count: 0,
            pr_count: 0,
            ci_links: vec![],
            git_commits: vec![],
            pr_links: vec![],
        });
    }

    bundles
}

#[allow(dead_code)] // planned dashboard helper - will be wired in upcoming UI integration
fn build_feature_media_assets(
    feature: &FeatureView,
    workpackages: &[WpView],
) -> Vec<MediaAssetView> {
    let mut media = vec![MediaAssetView {
        id: format!("media-{id}-cover", id = feature.id),
        source: "dashboard".into(),
        name: format!("{slug}-hero.png", slug = feature.slug),
        kind: "image".into(),
        mime: "image/png".into(),
        url_or_path: format!("/assets/{slug}/cover.png", slug = feature.slug),
        size_bytes: 128_512,
        uploaded_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    }];

    for wp in workpackages {
        media.push(MediaAssetView {
            id: format!("media-{fid}-wp-{wid}", fid = feature.id, wid = wp.id),
            source: "agent-work-package".into(),
            name: format!("{slug}-wp-{wid}.png", slug = feature.slug, wid = wp.id),
            kind: "screenshot".into(),
            mime: "image/png".into(),
            url_or_path: format!("/assets/wp/{wid}/coverage.png", wid = wp.id),
            size_bytes: 84_320 + (wp.id as usize * 3_000),
            uploaded_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        });
    }

    media
}

#[allow(dead_code)] // planned dashboard helper - will be wired in upcoming UI integration
fn build_feature_reports(
    feature: &FeatureView,
    workpackages: &[WpView],
) -> Vec<ReportArtifactView> {
    vec![ReportArtifactView {
        id: format!("report-{id}-coverage", id = feature.id),
        name: format!("Feature Coverage Report â€” {name}", name = feature.title),
        source: "coverage-engine".into(),
        status: "completed".into(),
        generated_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        rule_count: 5,
        satisfied_count: if feature.labels.is_empty() {
            2
        } else {
            feature.labels.len() + 2
        },
        compliant: !workpackages.is_empty(),
    }]
}

#[allow(dead_code)] // planned dashboard endpoint - will be wired in upcoming UI integration
pub async fn dashboard_page(
    State(state): State<SharedState>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let store = state.read().await;
    let filter = dashboard_filter_from_query(&query);
    let cards = build_kanban_cards(&store, filter);
    let (projects, active_project) = load_projects(&store);
    let active_filter = query.get("filter").cloned().unwrap_or_else(|| "all".into());
    render(DashboardPage {
        kanban_cards: cards,
        health: store.health.clone(),
        projects,
        active_project,
        active_filter,
    })
}

pub async fn kanban_board(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let store = state.read().await;
    let filter = dashboard_filter_from_query(&query);
    let cards = build_kanban_cards(&store, filter);
    let active_filter = query.get("filter").cloned().unwrap_or_else(|| "all".into());

    if is_htmx(&headers) {
        render(KanbanPartial { cards })
    } else {
        let (projects, active_project) = load_projects(&store);
        render(DashboardPage {
            kanban_cards: cards,
            health: store.health.clone(),
            projects,
            active_project,
            active_filter,
        })
    }
}

#[allow(dead_code)] // planned dashboard endpoint - will be wired in upcoming UI integration
pub async fn feature_detail(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
    _headers: HeaderMap,
) -> Response {
    let store = state.read().await;
    let feature = match store.features.iter().find(|f| f.id == id) {
        Some(f) => FeatureView::from_feature(f),
        None => return (StatusCode::NOT_FOUND, "Feature not found").into_response(),
    };
    let fid = feature.id;
    let wps: Vec<WpView> = store
        .work_packages
        .get(&id)
        .map(|v| v.iter().map(WpView::from_wp).collect())
        .unwrap_or_default();
    let events = build_feature_events(&feature, &wps);
    let evidence_bundles = build_feature_evidence_bundles(&feature, &wps);
    let media_assets = build_feature_media_assets(&feature, &wps);
    let reports = build_feature_reports(&feature, &wps);

    render(FeatureDetailPage {
        feature,
        feature_id: fid,
        workpackages: wps,
        events,
        evidence_bundles,
        media_assets,
        reports,
    })
}

pub async fn wp_list(State(state): State<SharedState>, Path(id): Path<i64>) -> Response {
    let store = state.read().await;
    let wps: Vec<WpView> = store
        .work_packages
        .get(&id)
        .map(|v| v.iter().map(WpView::from_wp).collect())
        .unwrap_or_default();
    render(WpListPartial {
        feature_id: id,
        workpackages: wps,
    })
}

#[allow(dead_code)] // planned dashboard endpoint - will be wired in upcoming UI integration
pub async fn health_panel(State(state): State<SharedState>) -> Response {
    let store = state.read().await;
    render(HealthPanelPartial {
        services: store.health.clone(),
    })
}

#[allow(dead_code)] // planned dashboard endpoint - will be wired in upcoming UI integration
pub async fn event_timeline(State(state): State<SharedState>) -> Response {
    let _ = state.read().await;
    render(EventTimelinePartial {
        feature_id: 0,
        events: vec![],
    })
}

#[allow(dead_code)] // planned dashboard endpoint - will be wired in upcoming UI integration
pub async fn agent_activity(_state: State<SharedState>) -> Response {
    let agents: Vec<AgentView> = vec![
        super::helpers::agent_view("spec-agent", "idle", "", "2m ago"),
        super::helpers::agent_view("impl-agent", "running", "WP13 implementation", "just now"),
    ];
    render(AgentActivityPartial { agents })
}

pub async fn project_switcher(State(state): State<SharedState>) -> Response {
    let store = state.read().await;
    let projects: Vec<ProjectView> = store
        .projects
        .iter()
        .map(|p| ProjectView {
            id: p.id,
            slug: p.slug.clone(),
            name: p.name.clone(),
            description: p.description.clone().unwrap_or_default(),
        })
        .collect();
    render(ProjectSwitcherPartial {
        projects,
        active_id: store.active_project_id,
    })
}

pub async fn switch_project(State(state): State<SharedState>, Path(id): Path<i64>) -> Response {
    {
        let mut store = state.write().await;
        if id == 0 {
            // id=0 means "All Projects" -- clear the filter.
            store.active_project_id = None;
        } else if store.projects.iter().any(|p| p.id == id) {
            store.active_project_id = Some(id);
        } else {
            return (StatusCode::NOT_FOUND, "Project not found").into_response();
        }
    }

    // Reload the kanban board with the updated project filter.
    let store = state.read().await;
    let cards = build_kanban_cards(&store, DashboardFilter::All);
    render(KanbanPartial { cards })
}

// â”€â”€ /api/time â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub async fn time_footer() -> axum::response::Html<String> {
    axum::response::Html(
        chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    )
}

// â”€â”€ SSE Stream /api/stream â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

use axum::response::sse::{Event, Sse};
use std::convert::Infallible;
use tokio::time::{Duration, interval};

/// GET /api/stream (Server-Sent Events)
/// Streams real-time feature and health updates to connected clients.
/// Broadcasts heartbeat with feature count and health status every 5 seconds.
pub async fn sse_stream(
    State(state): State<SharedState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let state = state.clone();
    let stream = async_stream::stream! {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let store = state.read().await;

            // Broadcast feature_updated event to refresh kanban
            let feature_count = store.features.len();
            let data = serde_json::json!({ "type": "heartbeat", "features": feature_count }).to_string();
            yield Ok(Event::default()
                .event("feature_updated")
                .data(data));

            // Broadcast health status
            let healthy_count = store.health.iter().filter(|s| s.healthy).count();
            let total_count = store.health.len();
            let health_data = serde_json::json!({
                "healthy": healthy_count,
                "total": total_count,
                "all_healthy": healthy_count == total_count
            }).to_string();
            yield Ok(Event::default()
                .event("health_changed")
                .data(health_data));
        }
    };
    Sse::new(stream)
}

// â”€â”€ /api/dashboard/work-packages.json â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// GET /api/dashboard/work-packages.json
/// Returns all work packages across all features as a flat JSON array.
/// Used by the React dashboard at port 5176 to populate the work-package store.
pub async fn all_work_packages_json(State(state): State<SharedState>) -> impl IntoResponse {
    let store = state.read().await;
    let work_packages: Vec<WorkPackageJson> = store
        .work_packages
        .iter()
        .flat_map(|(feature_id, wps)| {
            wps.iter().map(|wp| {
                let status = match wp.state {
                    agileplus_domain::domain::work_package::WpState::Planned => "planned",
                    agileplus_domain::domain::work_package::WpState::Doing => "in_progress",
                    agileplus_domain::domain::work_package::WpState::Review => "in_progress",
                    agileplus_domain::domain::work_package::WpState::Done => "completed",
                    agileplus_domain::domain::work_package::WpState::Blocked => "blocked",
                };
                WorkPackageJson {
                    id: wp.id.to_string(),
                    feature_id: *feature_id,
                    title: wp.title.clone(),
                    status: status.to_string(),
                    priority: "medium".to_string(),
                    assignee: wp.agent_id.clone(),
                }
            })
        })
        .collect();

    axum::Json(serde_json::json!({
        "work_packages": work_packages,
        "count": work_packages.len(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// â”€â”€ /api/dashboard/epics-stories.json â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// GET /api/dashboard/epics-stories.json
/// Reads Epics + Stories directly from the SQLite database and returns them
/// as a flat JSON payload. Used by the React dashboard at port 5176.
pub async fn epics_stories_json() -> impl IntoResponse {
    // Resolve db path: DATABASE_URL env â†’ DATABASE_PATH env â†’ default agileplus.db
    let db_path: PathBuf = if let Ok(url) = std::env::var("DATABASE_URL") {
        url.strip_prefix("sqlite:").unwrap_or(&url).into()
    } else if let Ok(p) = std::env::var("DATABASE_PATH") {
        PathBuf::from(p)
    } else {
        PathBuf::from("agileplus.db")
    };

    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            return axum::Json(serde_json::json!({
                "epics": [],
                "stories": [],
                "epic_count": 0,
                "story_count": 0,
                "error": format!("db open failed: {e}"),
            }));
        }
    };

    // Query epics
    let epics: Vec<serde_json::Value> = {
        let mut stmt = conn
            .prepare("SELECT id, title, status, requirement_id FROM epics ORDER BY id")
            .unwrap_or_else(|_| conn.prepare("SELECT 1 WHERE 0").unwrap());
        stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0).unwrap_or(0),
                "title": row.get::<_, String>(1).unwrap_or_default(),
                "status": row.get::<_, String>(2).unwrap_or_default(),
                "requirement_id": row.get::<_, Option<String>>(3).unwrap_or(None),
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    // Query stories
    let stories: Vec<serde_json::Value> = {
        let mut stmt = conn
            .prepare("SELECT id, epic_id, title, status, requirement_id FROM stories ORDER BY id")
            .unwrap_or_else(|_| conn.prepare("SELECT 1 WHERE 0").unwrap());
        stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0).unwrap_or(0),
                "epic_id": row.get::<_, Option<i64>>(1).unwrap_or(None),
                "title": row.get::<_, String>(2).unwrap_or_default(),
                "status": row.get::<_, String>(3).unwrap_or_default(),
                "requirement_id": row.get::<_, Option<String>>(4).unwrap_or(None),
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    let epic_count = epics.len();
    let story_count = stories.len();

    axum::Json(serde_json::json!({
        "epics": epics,
        "stories": stories,
        "epic_count": epic_count,
        "story_count": story_count,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
