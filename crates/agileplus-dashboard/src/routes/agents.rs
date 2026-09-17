//! Agent activity and configuration handlers for the dashboard.
//!
//! Provides real-time agent detection, JSON APIs, and configuration management
//! for the Claude Code agent pool. Routes handle both HTML (HTMX partials) and
//! JSON responses with automatic provider credential validation.

use std::env;

use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::app_state::SharedState;
use crate::process_detector;
use crate::templates::{AgentActivityPartial, AgentSettingsPage, AgentView, ToastPartial};

// ── JSON API Response Types ────────────────────────────────────────────────

/// JSON response for GET /api/dashboard/agents (real-time agent detection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub status: String,
    pub current_task: String,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub worktree: String,
    pub uptime: String,
}

// Configuration types are now imported from the settings module.

// ── Form Request Types ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AgentSettingsForm {
    pub pool_size: usize,
    pub retry_budget: usize,
    pub dispatch_mode: String,
    pub default_provider: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentTestConnectionForm {
    pub provider: String,
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Template rendering helper that converts Askama templates to HTML responses.
fn render<T: Template>(tpl: T) -> Response {
    match tpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            let status = axum::http::StatusCode::INTERNAL_SERVER_ERROR;
            (status, format!("Template error: {e}")).into_response()
        }
    }
}

/// Extract environment variable or return None if missing/empty.
fn env_or_none(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Calculate uptime string from the elapsed duration string produced by
/// `process_detector::get_process_start_time` (e.g. "5m", "1h 20m").
fn calculate_uptime(started_at: &Option<String>) -> String {
    match started_at {
        Some(elapsed) => format!("running for {elapsed}"),
        None => "uptime unknown".into(),
    }
}

// ── Handlers ───────────────────────────────────────────────────────────────

/// HTML: GET /api/dashboard/agents
/// Returns detected agent processes as an HTMX partial (polls every 5s from dashboard templates).
pub async fn agent_activity(State(state): State<SharedState>) -> Response {
    let _ = state.read().await;

    // Detect real agent processes
    let detected = process_detector::detect_agents();

    // Convert detected agents to view models
    let agents: Vec<AgentView> = detected
        .into_iter()
        .map(|agent| {
            let uptime = calculate_uptime(&agent.started_at);
            let worktree_label = agent
                .worktree
                .as_deref()
                .and_then(|wt| wt.split('/').next_back())
                .unwrap_or("")
                .to_string();
            let worktree = agent.worktree.unwrap_or_default();
            AgentView {
                name: agent.name,
                status: agent.status.clone(),
                current_task: agent.current_task,
                last_action: uptime,
                pid: Some(agent.pid),
                started_at: agent.started_at,
                worktree,
                worktree_label,
                is_live: agent.status == "running",
            }
        })
        .collect();

    render(AgentActivityPartial { agents })
}

/// JSON API: GET /api/dashboard/agents
/// Returns detected agent processes as JSON (polls every 5s from dashboard templates).
pub async fn agents_json(State(_state): State<SharedState>) -> impl IntoResponse {
    // Detect real agent processes
    let detected = process_detector::detect_agents();

    // Convert detected agents to JSON response
    let agents: Vec<AgentInfo> = detected
        .into_iter()
        .map(|agent| {
            let uptime = calculate_uptime(&agent.started_at);
            AgentInfo {
                name: agent.name,
                status: agent.status.clone(),
                current_task: agent.current_task,
                pid: Some(agent.pid),
                started_at: agent.started_at,
                worktree: agent.worktree.unwrap_or_default(),
                uptime,
            }
        })
        .collect();

    axum::Json(serde_json::json!({
        "agents": agents,
        "count": agents.len(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// HTML: GET /settings/agents
/// Returns the agent settings configuration page.
pub async fn agent_settings_page() -> Response {
    let config = super::settings::Config::load().unwrap_or(super::settings::Config {
        plane: None,
        agents: None,
        services: None,
        dashboard: None,
    });

    let agent_config = config
        .agents
        .unwrap_or_else(|| super::settings::AgentConfig {
            pool_size: 6,
            retry_budget: 3,
            dispatch_mode: "balanced".to_string(),
            default_provider: "claude".to_string(),
        });

    render(AgentSettingsPage {
        agent_pool_size: agent_config.pool_size,
        retry_budget: agent_config.retry_budget,
        dispatch_mode: agent_config.dispatch_mode,
        default_provider: agent_config.default_provider,
    })
}

/// POST: /api/settings/agents/test-connection
/// Validates that a given provider has required credentials available.
pub async fn test_agent_connection(
    axum::Form(form): axum::Form<AgentTestConnectionForm>,
) -> impl IntoResponse {
    // Provider reachability check: validate that required env vars are present.
    let (ok, msg) = match form.provider.as_str() {
        "claude" => {
            let key = env_or_none("ANTHROPIC_API_KEY");
            if key.is_some() {
                (
                    true,
                    "Claude API key detected — connection likely valid".to_string(),
                )
            } else {
                (false, "ANTHROPIC_API_KEY not set".to_string())
            }
        }
        "gemini" => {
            let key = env_or_none("GEMINI_API_KEY").or_else(|| env_or_none("GOOGLE_API_KEY"));
            if key.is_some() {
                (
                    true,
                    "Gemini API key detected — connection likely valid".to_string(),
                )
            } else {
                (false, "GEMINI_API_KEY / GOOGLE_API_KEY not set".to_string())
            }
        }
        "local" => (
            true,
            "Local provider requires no external credentials".to_string(),
        ),
        other => (false, format!("Unknown provider: {other}")),
    };

    let css = if ok { "text-green-400" } else { "text-red-400" };
    Html(format!(r#"<span class="{css}">{msg}</span>"#)).into_response()
}

/// POST: /api/settings/agents
/// Persists agent configuration (pool_size, retry_budget, dispatch_mode, default_provider).
pub async fn save_agent_settings(axum::Form(form): axum::Form<AgentSettingsForm>) -> Response {
    let mut config = match super::settings::Config::load() {
        Ok(c) => c,
        Err(error) => {
            return render(ToastPartial {
                message: format!("Failed to load settings safely: {error}"),
                success: false,
            });
        }
    };

    config.agents = Some(super::settings::AgentConfig {
        pool_size: form.pool_size,
        retry_budget: form.retry_budget,
        dispatch_mode: form.dispatch_mode.trim().to_string(),
        default_provider: form.default_provider.trim().to_string(),
    });

    match config.save() {
        Ok(_) => render(ToastPartial {
            message: "Agent settings saved successfully".to_string(),
            success: true,
        }),
        Err(e) => render(ToastPartial {
            message: format!("Failed to save settings: {e}"),
            success: false,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::super::settings::Config;

    #[test]
    fn malformed_config_is_not_replaced_by_agent_settings_defaults() {
        let config_path = std::env::temp_dir().join("agileplus-agent-malformed.toml");
        let malformed = "[agents\npool_size = 'not-an-integer'\n";
        std::fs::write(&config_path, malformed).unwrap();

        assert!(
            Config::load_from_path_with_credential_factory(&config_path, || {
                unreachable!("malformed config must fail before credential loading")
            })
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), malformed);
        std::fs::remove_file(config_path).unwrap();
    }
}
