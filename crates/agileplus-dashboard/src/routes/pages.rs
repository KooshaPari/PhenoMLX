//! Page route handlers for AgilePlus dashboard.
//!
//! Handlers for main page views (root, home, features, events, settings, hub, health, feature details).

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{Html, Response},
};
use std::collections::HashMap;

use crate::app_state::SharedState;
use crate::templates::{
    DashboardPage, EcosystemProject, EventsPage, FeatureView, FeaturesPage, HomePage, HubPage,
    SettingsPage,
};

use super::features;
use super::helpers::{self};

/// GET /
/// Home page with project summary statistics
pub async fn root(State(state): State<SharedState>) -> Response {
    let store = state.read().await;
    let total_features = store.features.len();
    let active_features = store
        .features
        .iter()
        .filter(|feature| {
            !matches!(
                feature.state,
                agileplus_domain::domain::state_machine::FeatureState::Shipped
                    | agileplus_domain::domain::state_machine::FeatureState::Retrospected
            )
        })
        .count();
    let shipped_features = store
        .features
        .iter()
        .filter(|feature| {
            matches!(
                feature.state,
                agileplus_domain::domain::state_machine::FeatureState::Shipped
                    | agileplus_domain::domain::state_machine::FeatureState::Retrospected
            )
        })
        .count();
    let projects = helpers::build_project_summaries(&store);

    helpers::render(HomePage {
        total_features,
        active_features,
        shipped_features,
        projects,
    })
}

/// GET /home
/// Alias for root page
pub async fn home(State(state): State<SharedState>) -> Response {
    root(State(state)).await
}

/// GET /dashboard
/// Dashboard page with kanban board
pub async fn dashboard_page(
    State(state): State<SharedState>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let store = state.read().await;
    let filter = helpers::dashboard_filter_from_query(&query);
    let cards = helpers::build_kanban_cards(&store, filter);
    let (projects, active_project) = helpers::load_projects(&store);
    let active_filter = query.get("filter").cloned().unwrap_or_else(|| "all".into());
    helpers::render(DashboardPage {
        kanban_cards: cards,
        health: store.health.clone(),
        projects,
        active_project,
        active_filter,
    })
}

/// GET /features
/// Features list page
pub async fn features_page(State(state): State<SharedState>) -> Response {
    let store = state.read().await;
    let features = store
        .features
        .iter()
        .map(FeatureView::from_feature)
        .collect::<Vec<_>>();
    helpers::render(FeaturesPage { features })
}

/// GET /events
/// Events timeline page
pub async fn events_page() -> Response {
    helpers::render(EventsPage {
        events: helpers::sample_events(),
    })
}

/// GET /settings
/// Settings page
pub async fn settings_page() -> Response {
    helpers::render(SettingsPage)
}

/// GET /hub
/// Ecosystem projects hub page
pub async fn hub_page() -> Response {
    let projects = vec![
        EcosystemProject {
            name: "phenodocs",
            tagline: "Ecosystem docs hub",
            stack: "TypeScript · Vue",
            port: Some(4100),
            github: "https://github.com/KooshaPari/phenodocs",
            category: "docs",
        },
        EcosystemProject {
            name: "AgilePlus",
            tagline: "Spec-driven PM platform",
            stack: "Rust · Tauri",
            port: Some(4101),
            github: "https://github.com/KooshaPari/AgilePlus",
            category: "app",
        },
        EcosystemProject {
            name: "heliosApp",
            tagline: "TypeScript runtime app",
            stack: "TypeScript · Bun",
            port: Some(4102),
            github: "https://github.com/KooshaPari/heliosApp",
            category: "app",
        },
        EcosystemProject {
            name: "thegent",
            tagline: "Agent framework",
            stack: "TypeScript · Python",
            port: Some(4103),
            github: "https://github.com/KooshaPari/thegent",
            category: "lib",
        },
        EcosystemProject {
            name: "bifrost-extensions",
            tagline: "LLM gateway extensions",
            stack: "Go",
            port: Some(4104),
            github: "https://github.com/KooshaPari/bifrost-extensions",
            category: "lib",
        },
        EcosystemProject {
            name: "civ",
            tagline: "CI validation",
            stack: "TypeScript",
            port: Some(4105),
            github: "https://github.com/KooshaPari/civ",
            category: "docs",
        },
        EcosystemProject {
            name: "TraceRTM",
            tagline: "Requirements traceability",
            stack: "Python · Go · TS",
            port: Some(4110),
            github: "https://github.com/KooshaPari/trace",
            category: "app",
        },
        EcosystemProject {
            name: "agentapi-plusplus",
            tagline: "Agent HTTP API",
            stack: "Go",
            port: None,
            github: "https://github.com/KooshaPari/agentapi-plusplus",
            category: "api",
        },
        EcosystemProject {
            name: "cliproxyapi-plusplus",
            tagline: "Multi-provider CLI proxy",
            stack: "Go",
            port: None,
            github: "https://github.com/KooshaPari/cliproxyapi-plusplus",
            category: "api",
        },
    ];
    helpers::render(HubPage { projects })
}

/// GET /features/:id
/// Feature detail page (full HTML page)
pub async fn feature_page(State(state): State<SharedState>, Path(id): Path<i64>) -> Response {
    features::feature_detail(State(state), Path(id), HeaderMap::new()).await
}

pub async fn time_footer() -> Html<String> {
    Html(
        chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    )
}
