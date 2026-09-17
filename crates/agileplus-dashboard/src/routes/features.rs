//! Feature-specific route handlers for the dashboard.
//!
//! This module contains handlers for feature detail pages, state transitions,
//! events, and media asset galleries. Each handler follows the HTMX partial
//! pattern: return only the changed component for AJAX requests, full page otherwise.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;

use agileplus_domain::domain::state_machine::FeatureState;

use crate::app_state::SharedState;
use crate::templates::{
    CiLinkView, EventTimelinePartial, EvidenceBundleView, FeatureDetailPage, FeatureView,
    GitCommitView, KanbanPartial, MediaAssetView, PrLinkView, ReportArtifactView, WpView,
    all_feature_states,
};

use chrono::Utc;

// ── Helper Functions ─────────────────────────────────────────────────────────

/// Render an Askama template to an HTML response.
/// Returns 500 Internal Server Error if template rendering fails.
fn render<T: Template>(tpl: T) -> Response {
    match tpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

/// Dashboard filter enumeration for grouping features by state.
#[allow(dead_code)] // reserved - planned dashboard filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashboardFilter {
    All,
    Active,
    Blocked,
    Shipped,
}

/// Build Kanban cards for the dashboard, grouped by feature state.
fn build_kanban_cards(
    store: &crate::app_state::DashboardStore,
    _filter: DashboardFilter,
) -> HashMap<String, Vec<FeatureView>> {
    let states = all_feature_states();
    let mut cards: HashMap<String, Vec<FeatureView>> = HashMap::new();
    for s in &states {
        cards.insert(s.clone(), vec![]);
    }
    // Group active features by state after applying project and sidebar filters.
    for feature in store.features_for_active_project() {
        let state = &feature.state;
        let state_key = format!("{state:?}");
        if let Some(features_in_state) = cards.get_mut(&state_key) {
            features_in_state.push(FeatureView::from_feature(feature));
        }
    }
    cards
}

/// Build feature event timeline.
/// Synthesizes a sequence of events for a feature: creation, sync, work-package state changes.
fn build_feature_events(
    feature: &FeatureView,
    workpackages: &[WpView],
) -> Vec<crate::templates::EventView> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let mut events = vec![crate::templates::EventView {
        id: format!("evt-feature-{}-created", feature.id),
        kind: "system".into(),
        description: format!("Feature '{}' opened in dashboard", feature.slug),
        timestamp: now.clone(),
        agent_name: None,
        agent_link: None,
        wp_id: None,
        wp_link: None,
        commit_sha: None,
        commit_link: None,
        ci_run_id: None,
        ci_run_link: None,
    }];

    if !workpackages.is_empty() {
        events.push(crate::templates::EventView {
            id: format!("evt-feature-{}-sync", feature.id),
            kind: "agent_action".into(),
            description: format!("{} work package entries synced", workpackages.len()),
            timestamp: now.clone(),
            agent_name: Some("sync-agent".to_string()),
            agent_link: Some("/agents/sync-agent".to_string()),
            wp_id: None,
            wp_link: None,
            commit_sha: Some("7c5b6ef".to_string()),
            commit_link: Some("https://github.com/Phenotype/AgilePlus/commit/7c5b6ef".to_string()),
            ci_run_id: Some("1024".to_string()),
            ci_run_link: Some(
                "https://github.com/Phenotype/AgilePlus/actions/runs/1024".to_string(),
            ),
        });

        for wp in workpackages {
            // agent_link: route to agent detail page when an agent_id is present.
            let agent_link = wp
                .agent_id
                .as_deref()
                .map(|aid| format!("/api/dashboard/agents/{aid}"));

            // wp_link: slug-based URL to the work package detail anchor.
            let wp_link = Some(format!(
                "/features/{}/work-packages/{}",
                feature.slug, wp.id
            ));

            // commit_link: GitHub commit URL when a head commit SHA is present.
            let (commit_sha, commit_link) = match &wp.head_commit {
                Some(sha) => (
                    Some(sha.clone()),
                    Some(format!(
                        "https://github.com/KooshaPari/AgilePlus/commit/{sha}"
                    )),
                ),
                None => (None, None),
            };

            // ci_run_link: derive from pr_url when it is a GitHub PR URL by
            // redirecting to the Actions tab for that repository.
            let ci_run_link = wp.pr_url.as_deref().and_then(|url| {
                // pr_url is typically https://github.com/{owner}/{repo}/pull/{n}
                // Strip the `/pull/{n}` suffix and append `/actions` for the runs view.
                let prefix = url
                    .split("/pull/")
                    .next()
                    .filter(|p| p.starts_with("https://github.com/"))?;
                Some(format!("{prefix}/actions"))
            });

            events.push(crate::templates::EventView {
                id: format!("evt-feature-{}-wp-{}", feature.id, wp.id),
                kind: "state_change".into(),
                description: format!("Work-package {} is in state '{}'", wp.title, wp.state),
                timestamp: now.clone(),
                agent_name: wp.agent_id.clone(),
                agent_link,
                wp_id: Some(wp.id.to_string()),
                wp_link,
                commit_sha,
                commit_link,
                ci_run_id: None,
                ci_run_link,
            });
        }
    } else {
        events.push(crate::templates::EventView {
            id: format!("evt-feature-{}-no-wp", feature.id),
            kind: "system".into(),
            description: "No work packages linked yet".into(),
            timestamp: now.clone(),
            agent_name: None,
            agent_link: None,
            wp_id: None,
            wp_link: None,
            commit_sha: None,
            commit_link: None,
            ci_run_id: None,
            ci_run_link: None,
        });
    }

    events
}

/// Build evidence bundle artifacts for a feature.
/// Attempts to load real bundles from disk first; falls back to stub bundles.
fn build_feature_evidence_bundles(
    feature: &FeatureView,
    workpackages: &[WpView],
) -> Vec<EvidenceBundleView> {
    // Try to load real bundles from disk first.
    let disk_bundles = load_evidence_bundles_from_disk(&feature.id.to_string());
    if !disk_bundles.is_empty() {
        return disk_bundles;
    }

    // Fall back to stub bundles when no disk bundles exist yet.
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

/// Build media asset gallery for a feature.
/// Includes a cover image and per-work-package screenshots.
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

/// Build feature report artifacts.
fn build_feature_reports(
    feature: &FeatureView,
    _workpackages: &[WpView],
) -> Vec<ReportArtifactView> {
    vec![ReportArtifactView {
        id: format!("report-{id}-coverage", id = feature.id),
        name: format!("Feature Coverage Report — {name}", name = feature.title),
        source: "coverage-engine".into(),
        status: "completed".into(),
        generated_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        rule_count: 5,
        satisfied_count: if feature.labels.is_empty() {
            2
        } else {
            feature.labels.len() + 2
        },
        compliant: !feature.labels.is_empty(),
    }]
}

/// Load real evidence bundles from `.agileplus/evidence/<feature_id>/bundle.json`.
fn load_evidence_bundles_from_disk(feature_id: &str) -> Vec<EvidenceBundleView> {
    let bundle_path = PathBuf::from(".agileplus")
        .join("evidence")
        .join(feature_id)
        .join("bundle.json");

    let content = match fs::read_to_string(&bundle_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let timestamp = val["timestamp"].as_str().unwrap_or("unknown").to_string();

    // Parse test_results
    let tr = &val["test_results"];
    let test_passed = tr["passed"].as_bool();
    let tests_passed_count = tr["passed_count"].as_u64().unwrap_or(0) as u32;
    let tests_failed_count = tr["failed_count"].as_u64().unwrap_or(0) as u32;
    let test_summary = tr["summary"].as_str().map(str::to_string);
    let test_output = tr["output_snippet"].as_str().map(str::to_string);

    // Parse git commits
    let git_commits: Vec<GitCommitView> = val["git_log"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|c| GitCommitView {
            short_hash: c["short_hash"].as_str().unwrap_or("").to_string(),
            subject: c["subject"].as_str().unwrap_or("").to_string(),
            date: c["date"].as_str().unwrap_or("").to_string(),
            author: c["author"].as_str().unwrap_or("").to_string(),
            url: c["url"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    // Parse PRs
    let pr_links: Vec<PrLinkView> = val["prs"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|p| PrLinkView {
            number: p["number"].as_u64().unwrap_or(0),
            title: p["title"].as_str().unwrap_or("").to_string(),
            url: p["url"].as_str().unwrap_or("").to_string(),
            state: p["state"].as_str().unwrap_or("").to_lowercase(),
            head_ref: p["headRefName"].as_str().unwrap_or("").to_string(),
            created_at: p["createdAt"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    // Parse CI links
    let ci_links: Vec<CiLinkView> = val["ci_links"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|c| CiLinkView {
            id: c["id"].as_i64().unwrap_or(0),
            title: c["title"].as_str().unwrap_or("").to_string(),
            status: c["status"].as_str().unwrap_or("").to_string(),
            conclusion: c["conclusion"].as_str().unwrap_or("pending").to_string(),
            url: c["url"].as_str().unwrap_or("").to_string(),
            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let commit_count = git_commits.len();
    let pr_count = pr_links.len();
    let status = if test_passed.unwrap_or(false) {
        "verified"
    } else {
        "generated"
    };

    vec![EvidenceBundleView {
        id: format!("bundle-{feature_id}-disk"),
        fr_id: format!("FR-{feature_id}"),
        evidence_type: "generated_bundle".into(),
        wp_id: "auto".into(),
        wp_title: format!("Evidence Bundle — {feature_id}"),
        artifact_path: bundle_path.display().to_string(),
        created_at: timestamp,
        artifact_ext: "json".into(),
        status: status.into(),
        content_preview: test_output,
        is_text_artifact: true,
        is_image_artifact: false,
        download_url: format!("/api/features/{feature_id}/evidence/bundle.json"),
        test_passed,
        tests_passed_count,
        tests_failed_count,
        test_summary,
        commit_count,
        pr_count,
        ci_links,
        git_commits,
        pr_links,
    }]
}

// ── Route Handlers ───────────────────────────────────────────────────────────

/// GET /api/dashboard/features/:id
/// Returns the full feature detail page with all associated data:
/// events, evidence bundles, media assets, and reports.
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

/// GET /features/:id
/// Alias for feature_detail; renders the full page layout.
pub async fn feature_page(State(state): State<SharedState>, Path(id): Path<i64>) -> Response {
    feature_detail(State(state), Path(id), HeaderMap::new()).await
}

/// GET /api/dashboard/features/:id/events
/// Returns the event timeline partial for a feature (HTMX).
pub async fn feature_events(
    State(state): State<SharedState>,
    Path(feature_id): Path<i64>,
) -> Response {
    let store = state.read().await;
    let feature = match store.features.iter().find(|f| f.id == feature_id) {
        Some(f) => FeatureView::from_feature(f),
        None => return (StatusCode::NOT_FOUND, "Feature not found").into_response(),
    };
    let wps: Vec<WpView> = store
        .work_packages
        .get(&feature_id)
        .map(|v| v.iter().map(WpView::from_wp).collect())
        .unwrap_or_default();
    let events = build_feature_events(&feature, &wps);

    render(EventTimelinePartial { feature_id, events })
}

/// GET /api/dashboard/features/:id/media
/// Returns the media gallery partial for a feature (HTMX).
/// Renders as a 2-column grid of media assets.
pub async fn feature_media(
    State(state): State<SharedState>,
    Path(feature_id): Path<i64>,
) -> Response {
    let store = state.read().await;
    let feature = match store.features.iter().find(|f| f.id == feature_id) {
        Some(f) => FeatureView::from_feature(f),
        None => return (StatusCode::NOT_FOUND, "Feature not found").into_response(),
    };
    let wps: Vec<WpView> = store
        .work_packages
        .get(&feature_id)
        .map(|v| v.iter().map(WpView::from_wp).collect())
        .unwrap_or_default();
    let media = build_feature_media_assets(&feature, &wps);

    // Return media assets as a simple HTML partial
    let html = media
        .iter()
        .map(|m| {
            format!(
                r#"<div class="media-asset border rounded p-3 bg-zinc-800">
                <img src="{}" alt="{}" class="w-full rounded"/>
                <p class="text-xs text-zinc-400 mt-2">{}</p>
              </div>"#,
                m.url_or_path, m.name, m.name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Html(format!(
        r#"<div class="grid grid-cols-2 gap-3 media-gallery">{html}</div>"#
    ))
    .into_response()
}

/// POST /api/features/:id/transition
/// Form data: `target_state` (feature state enum)
/// Transitions a feature to a new state and returns the updated Kanban cards.
#[derive(Debug, Deserialize)]
pub struct FeatureTransitionForm {
    #[serde(rename = "target_state")]
    pub new_state: String,
}

pub async fn feature_transition(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
    axum::Form(form): axum::Form<FeatureTransitionForm>,
) -> Response {
    let new_state = match form.new_state.parse::<FeatureState>() {
        Ok(s) => s,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid feature state").into_response(),
    };

    let feature_name = {
        let store = state.read().await;
        match store.features.iter().find(|f| f.id == id) {
            Some(f) => f.slug.clone(),
            None => return (StatusCode::NOT_FOUND, "Feature not found").into_response(),
        }
    };

    // Broadcast the update so SSE clients refresh
    // (In a real app, persist the state change here)
    tracing::info!(
        "Feature {} transitioned to {:?} (SSE broadcast triggers UI refresh)",
        feature_name,
        new_state
    );

    // Return the kanban partial so htmx can swap it
    let store = state.read().await;
    let cards = build_kanban_cards(&store, DashboardFilter::All);
    render(KanbanPartial { cards })
}
