// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::HashMap;
use std::env;

use agileplus_domain::domain::{
    feature::Feature, state_machine::FeatureState, work_package::WpState,
};
use askama::Template;
use axum::{
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};

use crate::app_state::DashboardStore;
use crate::templates::{FeatureView, ProjectSummaryView, ProjectView, all_feature_states};

#[allow(dead_code)] // reserved for Plane.so API integration
pub(super) const DEFAULT_PLANE_API_URL: &str = "https://app.plane.so";
#[allow(dead_code)] // reserved for Plane.so API integration
pub(super) const DEFAULT_PLANE_WEB_URL: &str = "https://app.plane.so";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DashboardFilter {
    All,
    Active,
    Blocked,
    Shipped,
}

pub(super) fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true")
        .unwrap_or(false)
}

pub(super) fn render<T: Template>(tpl: T) -> Response {
    match tpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

pub(super) fn load_projects(store: &DashboardStore) -> (Vec<ProjectView>, Option<ProjectView>) {
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
    let active_project = store.active_project().map(|p| ProjectView {
        id: p.id,
        slug: p.slug.clone(),
        name: p.name.clone(),
        description: p.description.clone().unwrap_or_default(),
    });
    (projects, active_project)
}

pub(super) fn build_project_summaries(store: &DashboardStore) -> Vec<ProjectSummaryView> {
    store
        .projects
        .iter()
        .map(|project| {
            let (feature_count, active_count, shipped_count) =
                store.feature_counts_for_project(project.id);
            ProjectSummaryView {
                project: ProjectView {
                    id: project.id,
                    slug: project.slug.clone(),
                    name: project.name.clone(),
                    description: project.description.clone().unwrap_or_default(),
                },
                feature_count,
                active_count,
                shipped_count,
            }
        })
        .collect()
}

#[allow(dead_code)] // utility - reserved for future env-based configuration
pub(super) fn env_or_none(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[allow(dead_code)] // utility - reserved for future env-based configuration
pub(super) fn parse_bool_env(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub(super) fn dashboard_filter_from_query(query: &HashMap<String, String>) -> DashboardFilter {
    match query.get("filter").map(|value| value.as_str()) {
        Some("active") => DashboardFilter::Active,
        Some("blocked") => DashboardFilter::Blocked,
        Some("shipped") => DashboardFilter::Shipped,
        _ => DashboardFilter::All,
    }
}

pub(super) fn feature_matches_filter(
    store: &DashboardStore,
    feature: &Feature,
    filter: DashboardFilter,
) -> bool {
    let is_blocked = store
        .work_packages
        .get(&feature.id)
        .map(|workpackages| workpackages.iter().any(|wp| wp.state == WpState::Blocked))
        .unwrap_or(false);

    match filter {
        DashboardFilter::All => true,
        DashboardFilter::Active => !matches!(
            feature.state,
            FeatureState::Shipped | FeatureState::Retrospected
        ),
        DashboardFilter::Blocked => is_blocked,
        DashboardFilter::Shipped => {
            matches!(
                feature.state,
                FeatureState::Shipped | FeatureState::Retrospected
            )
        }
    }
}

pub(super) fn build_kanban_cards(
    store: &DashboardStore,
    filter: DashboardFilter,
) -> HashMap<String, Vec<FeatureView>> {
    let states = all_feature_states();
    let mut cards: HashMap<String, Vec<FeatureView>> = HashMap::new();
    for s in &states {
        cards.insert(s.clone(), vec![]);
    }
    for feature in store.features_for_active_project() {
        if !feature_matches_filter(store, feature, filter) {
            continue;
        }
        let state_key = feature.state.to_string();
        let view = FeatureView::from_feature(feature);
        cards.entry(state_key).or_default().push(view);
    }
    cards
}

pub(super) fn event_view(
    id: impl Into<String>,
    kind: impl Into<String>,
    description: impl Into<String>,
    timestamp: impl Into<String>,
) -> crate::templates::EventView {
    crate::templates::EventView {
        id: id.into(),
        kind: kind.into(),
        description: description.into(),
        timestamp: timestamp.into(),
        agent_name: None,
        agent_link: None,
        wp_id: None,
        wp_link: None,
        commit_sha: None,
        commit_link: None,
        ci_run_id: None,
        ci_run_link: None,
    }
}

pub(super) fn agent_view(
    name: impl Into<String>,
    status: impl Into<String>,
    current_task: impl Into<String>,
    last_action: impl Into<String>,
) -> crate::templates::AgentView {
    crate::templates::AgentView {
        name: name.into(),
        status: status.into(),
        current_task: current_task.into(),
        last_action: last_action.into(),
        pid: None,
        started_at: None,
        worktree: String::new(),
        worktree_label: String::new(),
        is_live: false,
    }
}

pub(super) fn sample_events() -> Vec<crate::templates::EventView> {
    vec![
        event_view(
            "evt-1",
            "system",
            "Dashboard booted with native Plane surface",
            "just now",
        ),
        event_view(
            "evt-2",
            "agent_action",
            "Planner synced feature ownership metadata",
            "2m ago",
        ),
        event_view(
            "evt-3",
            "state_change",
            "Feature moved from researched to planned",
            "9m ago",
        ),
    ]
}

// ── HTML and URL utilities ─────────────────────────────────────────────────

/// Minimal HTML entity escaping for embedding text content in HTML attributes/elements.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Classify a file extension into a broad artifact type for display purposes.
#[allow(dead_code)] // utility - reserved for future artifact display features
pub fn artifact_type_for_ext(ext: &str) -> &'static str {
    match ext {
        "lcov" | "coverage" | "cov" => "coverage",
        "xml" | "junit" | "tap" => "test-results",
        "json" | "sarif" => "report",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => "image",
        "md" | "txt" | "log" => "text",
        _ => "artifact",
    }
}

/// Percent-encode path segments so they are safe to embed in URLs.
///
/// Only encodes characters that are not allowed unencoded in URL path segments:
/// spaces, `#`, `?`, `%`, and `+`.
#[allow(dead_code)] // utility - reserved for future artifact display features
pub fn percent_encode_path(path: &str) -> String {
    path.chars()
        .flat_map(|c| match c {
            ' ' => vec!['%', '2', '0'],
            '#' => vec!['%', '2', '3'],
            '?' => vec!['%', '3', 'F'],
            '%' => vec!['%', '2', '5'],
            '+' => vec!['%', '2', 'B'],
            other => vec![other],
        })
        .collect()
}

// ── Service restart command validation ─────────────────────────────────────

const ALLOWED_RESTART_PROGRAMS: [&str; 4] = ["systemctl", "docker", "process-compose", "echo"];

pub fn is_restart_command_allowed(program: &str) -> bool {
    ALLOWED_RESTART_PROGRAMS.contains(&program)
}

pub fn validate_restart_command(cmd_line: &str) -> Result<(), String> {
    let mut parts: Vec<&str> = cmd_line.split_whitespace().collect();
    if parts.is_empty() {
        return Err("empty restart command".into());
    }

    let program = parts.remove(0);
    if !is_restart_command_allowed(program) {
        return Err(format!(
            "command '{program}' is not in approved restart command registry: {ALLOWED_RESTART_PROGRAMS:?}"
        ));
    }

    Ok(())
}

pub fn build_restart_command(cmd_line: &str) -> Result<std::process::Command, String> {
    validate_restart_command(cmd_line)?;

    let mut parts: Vec<&str> = cmd_line.split_whitespace().collect();
    let program = parts.remove(0);

    let mut cmd = std::process::Command::new(program);
    if !parts.is_empty() {
        cmd.args(parts);
    }
    Ok(cmd)
}
