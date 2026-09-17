//! Settings handlers for plane, agents, services, and dashboard configuration.
//!
//! Provides routes for displaying and persisting configuration across the dashboard.
//! Includes form handlers for plane sync, agent pool, service endpoints, and theme/logging
//! preferences. Each handler validates inputs before persisting to the local config file.

use std::env;
use std::path::Path;

use agileplus_domain::config::AppConfig;
use agileplus_domain::credentials::{CredentialStore, PLANESO_KEY, create_credential_store};
use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::app_state::SharedState;
use crate::templates::{
    PlaneHealthEndpointView, PlaneSettingsPage, ServicesSettingsPage, SettingsPage, ToastPartial,
};

// ── Configuration Types ────────────────────────────────────────────────────

/// Plane sync configuration (API endpoint, credentials, workspace/project slugs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaneConfig {
    pub api_url: String,
    /// Reference to the credential-store entry; never a secret value.
    #[serde(default = "default_plane_api_key_ref")]
    pub api_key_ref: String,
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    pub workspace_slug: String,
    pub project_slug: String,
}

fn default_plane_api_key_ref() -> String {
    PLANESO_KEY.to_string()
}

/// Agent pool configuration (size, retry budget, dispatch strategy, LLM provider).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub pool_size: usize,
    pub retry_budget: usize,
    pub dispatch_mode: String,
    pub default_provider: String,
}

/// Single service endpoint configuration (name, URL, optional timeout/retry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub endpoint_url: String,
    #[serde(default = "default_service_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

pub fn default_service_enabled() -> bool {
    true
}

/// Dashboard UI configuration (theme, logging level, data directory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub theme: String,
    pub log_level: String,
    pub data_directory: String,
}

/// Composite configuration container (reads/writes to ~/.agileplus/config.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub plane: Option<PlaneConfig>,
    pub agents: Option<AgentConfig>,
    pub services: Option<Vec<ServiceConfig>>,
    pub dashboard: Option<DashboardConfig>,
}

impl Config {
    /// Move a legacy inline Plane key into the configured credential backend.
    /// The source field is scrubbed only after storage succeeds.
    pub fn migrate_legacy_plane_key(
        &mut self,
        credentials: &dyn CredentialStore,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(plane) = self.plane.as_mut() else {
            return Ok(false);
        };
        let Some(legacy_key) = plane.api_key.as_deref().filter(|key| !key.is_empty()) else {
            return Ok(false);
        };
        credentials.set("agileplus", PLANESO_KEY, legacy_key)?;
        plane.api_key = None;
        plane.api_key_ref = PLANESO_KEY.to_string();
        Ok(true)
    }
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path();
        Self::load_from_path_with_credential_factory(&config_path, || {
            let app_config = AppConfig::load().map_err(|error| {
                agileplus_domain::credentials::CredentialError::BackendError(error.to_string())
            })?;
            create_credential_store(&app_config)
        })
    }

    fn empty() -> Self {
        Self {
            plane: None,
            agents: None,
            services: None,
            dashboard: None,
        }
    }

    fn read_from_path(config_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(config_path)?;
        Ok(toml::from_str(&content)?)
    }

    fn has_legacy_plane_key(&self) -> bool {
        self.plane
            .as_ref()
            .and_then(|plane| plane.api_key.as_deref())
            .is_some_and(|key| !key.is_empty())
    }

    pub(crate) fn load_from_path_with_credential_factory<F>(
        config_path: &Path,
        credential_factory: F,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: FnOnce() -> Result<
            Box<dyn CredentialStore>,
            agileplus_domain::credentials::CredentialError,
        >,
    {
        if !config_path.exists() {
            return Ok(Self::empty());
        }
        let mut config = Self::read_from_path(config_path)?;
        if config.has_legacy_plane_key() {
            // Do not rewrite the config until the secure store accepts the
            // secret. A failed migration leaves the raw legacy document intact.
            let credentials = credential_factory()?;
            let previous_credential = match credentials.get("agileplus", PLANESO_KEY) {
                Ok(value) => Some(value),
                Err(agileplus_domain::credentials::CredentialError::NotFound(_)) => None,
                Err(error) => return Err(Box::new(error)),
            };
            config.migrate_legacy_plane_key(credentials.as_ref())?;
            if let Err(error) = config.save_to_path(config_path) {
                restore_credential(credentials.as_ref(), previous_credential.as_deref())?;
                return Err(error);
            }
        }
        Ok(config)
    }

    pub(crate) fn save_to_path(
        &self,
        config_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to_path(&Self::config_path())
    }

    pub(crate) fn config_path() -> std::path::PathBuf {
        std::env::var("HOME")
            .ok()
            .map(|home| std::path::PathBuf::from(home).join(".agileplus/config.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from(".agileplus/config.toml"))
    }
}

/// Restore a credential after a later config write fails. This keeps a failed
/// migration or settings save from changing the effective credential state.
fn restore_credential(
    credentials: &dyn CredentialStore,
    previous: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match previous {
        Some(value) => credentials.set("agileplus", PLANESO_KEY, value)?,
        None => match credentials.delete("agileplus", PLANESO_KEY) {
            Ok(()) | Err(agileplus_domain::credentials::CredentialError::NotFound(_)) => {}
            Err(error) => return Err(Box::new(error)),
        },
    }
    Ok(())
}

// ── Form Request Types ────────────────────────────────────────────────────

/// Form data for plane sync settings (from HTML form POST).
#[derive(Debug, Deserialize)]
pub struct PlaneSettingsForm {
    pub api_url: String,
    pub api_key: String,
    pub workspace_slug: String,
    pub project_slug: String,
}

/// Form data for agent pool configuration (from HTML form POST).
#[derive(Debug, Deserialize)]
pub struct AgentSettingsForm {
    pub pool_size: usize,
    pub retry_budget: usize,
    pub dispatch_mode: String,
    pub default_provider: String,
}

/// Form data for custom service endpoint addition (from HTML form POST).
#[derive(Debug, Deserialize)]
pub struct ServiceSettingsForm {
    pub names: Vec<String>,
    pub endpoint_urls: Vec<String>,
}

/// Form data for dashboard UI settings (from HTML form POST).
#[derive(Debug, Deserialize)]
pub struct DashboardSettingsForm {
    pub theme: String,
    pub log_level: String,
    pub data_directory: String,
}

/// Form data for single service endpoint validation (from HTML form POST).
#[derive(Debug, Deserialize)]
pub struct SingleServiceTestForm {
    pub name: String,
    pub endpoint_url: String,
}

// ── Helper Functions ──────────────────────────────────────────────────────

const DEFAULT_PLANE_API_URL: &str = "https://app.plane.so";
const DEFAULT_PLANE_WEB_URL: &str = "https://app.plane.so";

/// Extract environment variable or return None if missing/empty.
fn env_or_none(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Format API key as hint string (first and last char with dots for obfuscation).
pub(crate) fn plane_api_key_hint(api_key: &Option<String>) -> String {
    match api_key {
        Some(key) => match (key.chars().next(), key.chars().next_back()) {
            (Some(first), Some(last)) => format!("{first}••••••{last}"),
            _ => "Configured".to_string(),
        },
        None => "Not configured".to_string(),
    }
}

/// Filter service health records to plane-related endpoints and convert to view models.
fn plane_health_endpoints(
    services: &[crate::app_state::ServiceHealth],
) -> Vec<PlaneHealthEndpointView> {
    services
        .iter()
        .filter(|service| service.name.contains("Plane") || service.name.starts_with("API"))
        .map(|service| PlaneHealthEndpointView {
            name: service.name.clone(),
            healthy: service.healthy,
            degraded: service.degraded,
            latency_ms: service.latency_ms,
            last_check_utc: service
                .last_check
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        })
        .collect()
}

/// Determine plane sync mode from environment variable (bidirectional or one-way).
fn plane_sync_mode() -> String {
    let bidirectional = env::var("PLANE_SYNC_BIDIRECTIONAL")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    if bidirectional {
        "Bidirectional".to_string()
    } else {
        "One-way".to_string()
    }
}

/// Validate plane connection status by checking required configuration fields.
pub(crate) fn plane_connection_checks(
    api_key: &Option<String>,
    workspace: &Option<String>,
) -> (bool, String, Vec<String>) {
    let mut warnings = Vec::new();
    if api_key.is_none() {
        warnings.push("Missing PLANE_API_KEY; configure a valid Plane API key".to_string());
    }
    if workspace.is_none() {
        warnings.push("Missing PLANE_WORKSPACE; set workspace slug for Plane sync".to_string());
    }

    if warnings.is_empty() {
        (true, "Connected via PLANE_API_KEY".to_string(), warnings)
    } else if warnings.len() == 1 {
        let status = warnings[0].clone();
        (false, status, warnings)
    } else {
        (false, "Plane settings incomplete".to_string(), warnings)
    }
}

/// Format coverage percentage as human-readable string (e.g. "5/10 (50%)").
pub(crate) fn percentage_coverage(hit: usize, total: usize) -> String {
    if total == 0 {
        return "0/0 (0%)".to_string();
    }
    let ratio = (hit.saturating_mul(100)).saturating_div(total);
    format!("{hit}/{total} ({ratio}%)")
}

/// Template rendering helper that converts Askama templates to HTML responses.
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

// ── Route Handlers ────────────────────────────────────────────────────────

/// GET /settings
/// Returns the settings overview page (links to all settings sections).
pub async fn settings_page() -> Response {
    render(SettingsPage)
}

/// GET /settings/plane
/// Returns the Plane sync configuration page with connection status and health metrics.
pub async fn plane_settings_page(State(state): State<SharedState>) -> Response {
    let store = state.read().await;
    let plane_workspace = env_or_none("PLANE_WORKSPACE");
    let project_slug = env_or_none("PLANE_PROJECT").unwrap_or_else(|| "not configured".to_string());
    let plane_api_key = env_or_none("PLANE_API_KEY");
    let plane_api_url =
        env_or_none("PLANE_API_URL").unwrap_or_else(|| DEFAULT_PLANE_API_URL.to_string());
    let plane_web_url =
        env_or_none("PLANE_WEB_URL").unwrap_or_else(|| DEFAULT_PLANE_WEB_URL.to_string());
    let (connected, connection_status, mut config_warnings) =
        plane_connection_checks(&plane_api_key, &plane_workspace);

    let plane_health_endpoints = plane_health_endpoints(&store.health);
    let plane_health_healthy = plane_health_endpoints
        .iter()
        .all(|endpoint| endpoint.healthy && !endpoint.degraded);
    let plane_api_latency_ms = plane_health_endpoints
        .iter()
        .find(|endpoint| endpoint.name == "Plane API")
        .and_then(|endpoint| endpoint.latency_ms);

    if !connected {
        config_warnings
            .push("Plane sync disabled until required settings are provided".to_string());
    }

    if !plane_health_healthy {
        config_warnings.push("Plane API health check is not healthy".to_string());
    }

    let mapped_features = store
        .features
        .iter()
        .filter(|feature| feature.plane_issue_id.is_some())
        .count();
    let total_features = store.features.len();
    let mapped_work_packages = store
        .work_packages
        .values()
        .flatten()
        .filter(|wp| wp.plane_sub_issue_id.is_some())
        .count();
    let total_work_packages: usize = store.work_packages.values().map(Vec::len).sum();

    let connection_status_configured = !connection_status.is_empty();

    render(PlaneSettingsPage {
        workspace_name: plane_workspace
            .clone()
            .unwrap_or_else(|| "Not configured".to_string()),
        workspace_slug: plane_workspace.unwrap_or_else(|| "not configured".to_string()),
        project_slug,
        plane_api_url: plane_api_url.trim_end_matches('/').to_string(),
        plane_web_url: plane_web_url.trim_end_matches('/').to_string(),
        plane_api_url_set: !plane_api_url.trim_end_matches('/').is_empty(),
        plane_web_url_set: !plane_web_url.trim_end_matches('/').is_empty(),
        plane_api_key_hint: plane_api_key_hint(&plane_api_key),
        plane_api_key_set: plane_api_key.is_some(),
        sync_enabled: connected,
        sync_mode: plane_sync_mode(),
        connected,
        connection_status: connection_status.clone(),
        connection_status_configured,
        plane_service_healthy: plane_health_healthy,
        plane_api_latency_ms,
        plane_health_endpoints,
        mapped_features_coverage: percentage_coverage(mapped_features, total_features),
        mapped_work_packages_coverage: percentage_coverage(
            mapped_work_packages,
            total_work_packages,
        ),
        mapped_features,
        mapped_work_packages,
        config_warnings,
    })
}

/// GET /settings/agents
/// Returns the agent pool configuration page (size, retry budget, dispatch mode, provider).
pub async fn agent_settings_page() -> Response {
    let config = Config::load().unwrap_or(Config {
        plane: None,
        agents: None,
        services: None,
        dashboard: None,
    });

    let agent_config = config.agents.unwrap_or_else(|| AgentConfig {
        pool_size: 6,
        retry_budget: 3,
        dispatch_mode: "balanced".to_string(),
        default_provider: "claude".to_string(),
    });

    render(crate::templates::AgentSettingsPage {
        agent_pool_size: agent_config.pool_size,
        retry_budget: agent_config.retry_budget,
        dispatch_mode: agent_config.dispatch_mode,
        default_provider: agent_config.default_provider,
    })
}

/// GET /settings/services
/// Returns the custom service endpoints configuration page.
pub async fn services_settings_page(State(state): State<SharedState>) -> Response {
    let store = state.read().await;
    let config = Config::load().unwrap_or(Config {
        plane: None,
        agents: None,
        services: None,
        dashboard: None,
    });

    let configs: Vec<crate::templates::ServiceConfigView> = config
        .services
        .unwrap_or_default()
        .into_iter()
        .map(|s| crate::templates::ServiceConfigView {
            name: s.name,
            endpoint_url: s.endpoint_url,
        })
        .collect();

    render(ServicesSettingsPage {
        services: store.health.clone(),
        configs,
    })
}

// ── Settings POST Handlers ─────────────────────────────────────────────────

/// POST /api/settings/plane
/// Persists plane sync configuration to the local config file.
pub async fn save_plane_settings(axum::Form(form): axum::Form<PlaneSettingsForm>) -> Response {
    let api_key = form.api_key.trim();
    if api_key.is_empty() {
        return render(ToastPartial {
            message: "Plane API key is required".to_string(),
            success: false,
        });
    }
    let credentials = match AppConfig::load()
        .map_err(|error| {
            agileplus_domain::credentials::CredentialError::BackendError(error.to_string())
        })
        .and_then(|app_config| create_credential_store(&app_config))
    {
        Ok(store) => store,
        Err(error) => {
            return render(ToastPartial {
                message: format!("Failed to securely store Plane credential: {error}"),
                success: false,
            });
        }
    };
    match persist_plane_settings(&form, &Config::config_path(), credentials.as_ref()) {
        Ok(()) => render(ToastPartial {
            message: "Plane settings saved successfully".to_string(),
            success: true,
        }),
        Err(error) => render(ToastPartial {
            message: format!("Failed to save settings: {error}"),
            success: false,
        }),
    }
}

fn persist_plane_settings(
    form: &PlaneSettingsForm,
    config_path: &Path,
    credentials: &dyn CredentialStore,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = if config_path.exists() {
        Config::read_from_path(config_path)?
    } else {
        Config::empty()
    };
    let previous_credential = match credentials.get("agileplus", PLANESO_KEY) {
        Ok(value) => Some(value),
        Err(agileplus_domain::credentials::CredentialError::NotFound(_)) => None,
        Err(error) => return Err(Box::new(error)),
    };
    config.migrate_legacy_plane_key(credentials)?;
    credentials.set("agileplus", PLANESO_KEY, form.api_key.trim())?;

    config.plane = Some(PlaneConfig {
        api_url: form.api_url.trim().to_string(),
        api_key_ref: PLANESO_KEY.to_string(),
        api_key: None,
        workspace_slug: form.workspace_slug.trim().to_string(),
        project_slug: form.project_slug.trim().to_string(),
    });

    if let Err(error) = config.save_to_path(config_path) {
        restore_credential(credentials, previous_credential.as_deref())?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::credentials::CredentialError;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temporary_config_path() -> std::path::PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("agileplus-dashboard-settings-{id}"))
            .join(".agileplus")
            .join("config.toml")
    }

    fn write_legacy_fixture(path: &Path) -> String {
        let fixture = concat!(
            "[plane]\n",
            "api_url = 'https://plane.example'\n",
            "api_key = 'raw-legacy-plane-secret'\n",
            "workspace_slug = 'workspace'\n",
            "project_slug = 'project'\n"
        )
        .to_string();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &fixture).unwrap();
        fixture
    }

    struct RejectingCredentialStore;

    impl CredentialStore for RejectingCredentialStore {
        fn get(&self, _service: &str, _key: &str) -> Result<String, CredentialError> {
            Err(CredentialError::BackendError("unavailable".to_string()))
        }

        fn set(&self, _service: &str, _key: &str, _value: &str) -> Result<(), CredentialError> {
            Err(CredentialError::BackendError("unavailable".to_string()))
        }

        fn delete(&self, _service: &str, _key: &str) -> Result<(), CredentialError> {
            Err(CredentialError::BackendError("unavailable".to_string()))
        }

        fn list_keys(&self, _service: &str) -> Result<Vec<String>, CredentialError> {
            Err(CredentialError::BackendError("unavailable".to_string()))
        }
    }

    #[test]
    fn plane_config_serialization_never_contains_api_key_value() {
        let config = Config {
            plane: Some(PlaneConfig {
                api_url: "https://plane.example".to_string(),
                api_key_ref: PLANESO_KEY.to_string(),
                api_key: None,
                workspace_slug: "workspace".to_string(),
                project_slug: "project".to_string(),
            }),
            agents: None,
            services: None,
            dashboard: None,
        };
        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("api_key_ref"));
        assert!(!serialized.contains("plane-secret-value"));
        assert!(!serialized.contains("api_key ="));
    }

    #[test]
    fn legacy_plane_key_is_moved_then_scrubbed() {
        let store = agileplus_domain::credentials::InMemoryCredentialStore::new();
        let mut config = Config {
            plane: Some(PlaneConfig {
                api_url: "https://plane.example".into(),
                api_key_ref: String::new(),
                api_key: Some("legacy-plane-secret".into()),
                workspace_slug: "workspace".into(),
                project_slug: "project".into(),
            }),
            agents: None,
            services: None,
            dashboard: None,
        };
        assert!(config.migrate_legacy_plane_key(&store).unwrap());
        assert_eq!(
            store.get("agileplus", PLANESO_KEY).unwrap(),
            "legacy-plane-secret"
        );
        let serialized = toml::to_string(&config).unwrap();
        assert!(!serialized.contains("legacy-plane-secret"));
    }

    #[test]
    fn legacy_plane_toml_deserializes_with_default_reference() {
        let config: Config = toml::from_str(
            "[plane]\napi_url = 'https://plane.example'\napi_key = 'legacy-secret'\nworkspace_slug = 'workspace'\nproject_slug = 'project'\n",
        )
        .unwrap();
        let plane = config.plane.unwrap();
        assert_eq!(plane.api_key_ref, PLANESO_KEY);
        assert_eq!(plane.api_key.as_deref(), Some("legacy-secret"));
    }

    #[test]
    fn raw_legacy_fixture_loads_migrates_and_scrubs_without_losing_config() {
        let config_path = temporary_config_path();
        write_legacy_fixture(&config_path);

        let config = Config::load_from_path_with_credential_factory(&config_path, || {
            Ok(Box::new(
                agileplus_domain::credentials::InMemoryCredentialStore::new(),
            ))
        })
        .unwrap();
        let plane = config.plane.unwrap();
        assert_eq!(plane.api_url, "https://plane.example");
        assert_eq!(plane.workspace_slug, "workspace");
        assert_eq!(plane.project_slug, "project");
        assert_eq!(plane.api_key, None);
        let persisted = std::fs::read_to_string(&config_path).unwrap();
        assert!(persisted.contains("api_key_ref"));
        assert!(!persisted.contains("raw-legacy-plane-secret"));
        std::fs::remove_dir_all(config_path.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn failed_load_time_migration_preserves_raw_legacy_fixture() {
        let config_path = temporary_config_path();
        let fixture = write_legacy_fixture(&config_path);

        assert!(
            Config::load_from_path_with_credential_factory(&config_path, || {
                Ok(Box::new(RejectingCredentialStore))
            })
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), fixture);
        std::fs::remove_dir_all(config_path.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn malformed_config_fails_closed_without_replacing_source_bytes() {
        let config_path = temporary_config_path();
        let malformed = "[plane\napi_url = 'not valid toml'\n";
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, malformed).unwrap();
        let store = agileplus_domain::credentials::InMemoryCredentialStore::new();
        let form = PlaneSettingsForm {
            api_url: "https://plane.example".into(),
            api_key: "new-secret".into(),
            workspace_slug: "workspace".into(),
            project_slug: "project".into(),
        };

        assert!(persist_plane_settings(&form, &config_path, &store).is_err());
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), malformed);
        assert!(matches!(
            store.get("agileplus", PLANESO_KEY),
            Err(CredentialError::NotFound(_))
        ));
        std::fs::remove_dir_all(config_path.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn failed_plane_config_write_does_not_mutate_credential() {
        let root = std::env::temp_dir().join("agileplus-dashboard-save-failure");
        let parent_file = root.join("not-a-directory");
        let config_path = parent_file.join("config.toml");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&parent_file, "sentinel").unwrap();
        let store = agileplus_domain::credentials::InMemoryCredentialStore::new();
        store.set("agileplus", PLANESO_KEY, "old-secret").unwrap();
        let form = PlaneSettingsForm {
            api_url: "https://plane.example".into(),
            api_key: "new-secret".into(),
            workspace_slug: "workspace".into(),
            project_slug: "project".into(),
        };

        assert!(persist_plane_settings(&form, &config_path, &store).is_err());
        assert_eq!(store.get("agileplus", PLANESO_KEY).unwrap(), "old-secret");
        assert_eq!(std::fs::read_to_string(&parent_file).unwrap(), "sentinel");
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// POST /api/settings/agents
/// Persists agent configuration (pool_size, retry_budget, dispatch_mode, default_provider).
pub async fn save_agent_settings(axum::Form(form): axum::Form<AgentSettingsForm>) -> Response {
    let mut config = match Config::load() {
        Ok(c) => c,
        Err(error) => {
            return render(ToastPartial {
                message: format!("Failed to load settings safely: {error}"),
                success: false,
            });
        }
    };

    config.agents = Some(AgentConfig {
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

/// POST /api/settings/dashboard
/// Persists dashboard UI configuration (theme, log_level, data_directory).
pub async fn save_dashboard_settings(
    axum::Form(form): axum::Form<DashboardSettingsForm>,
) -> Response {
    let mut config = match Config::load() {
        Ok(c) => c,
        Err(error) => {
            return render(ToastPartial {
                message: format!("Failed to load settings safely: {error}"),
                success: false,
            });
        }
    };

    config.dashboard = Some(DashboardConfig {
        theme: form.theme.trim().to_string(),
        log_level: form.log_level.trim().to_string(),
        data_directory: form.data_directory.trim().to_string(),
    });

    match config.save() {
        Ok(_) => render(ToastPartial {
            message: "Dashboard settings saved successfully".to_string(),
            success: true,
        }),
        Err(e) => render(ToastPartial {
            message: format!("Failed to save settings: {e}"),
            success: false,
        }),
    }
}

/// POST /api/settings/services
/// Persists custom service endpoint configuration.
pub async fn save_services_settings(axum::Form(form): axum::Form<ServiceSettingsForm>) -> Response {
    let mut config = match Config::load() {
        Ok(c) => c,
        Err(error) => {
            return render(ToastPartial {
                message: format!("Failed to load settings safely: {error}"),
                success: false,
            });
        }
    };

    let mut services = Vec::new();
    for (name, url) in form.names.into_iter().zip(form.endpoint_urls) {
        if !name.trim().is_empty() {
            services.push(ServiceConfig {
                name: name.trim().to_string(),
                endpoint_url: url.trim().to_string(),
                enabled: default_service_enabled(),
                timeout_ms: None,
                max_retries: None,
            });
        }
    }
    config.services = Some(services);

    match config.save() {
        Ok(_) => render(ToastPartial {
            message: "Service settings saved successfully".to_string(),
            success: true,
        }),
        Err(e) => render(ToastPartial {
            message: format!("Failed to save settings: {e}"),
            success: false,
        }),
    }
}

// ── Connection Testing Handlers ────────────────────────────────────────────

/// POST /api/settings/services/test
/// Validates a single service endpoint (basic URL format check).
pub async fn test_service_connection(
    axum::Form(form): axum::Form<SingleServiceTestForm>,
) -> Response {
    let is_valid = !form.endpoint_url.trim().is_empty() && form.endpoint_url.starts_with("http");

    if is_valid {
        render(ToastPartial {
            message: format!("Connection to {} successful (mock)", form.name),
            success: true,
        })
    } else {
        render(ToastPartial {
            message: format!("Invalid endpoint for {}: {}", form.name, form.endpoint_url),
            success: false,
        })
    }
}

/// POST /api/settings/plane/test
/// Validates plane connection (checks required fields and URL format).
pub async fn test_plane_connection(axum::Form(form): axum::Form<PlaneSettingsForm>) -> Response {
    // Simple validation: check that required fields are filled and api_url looks like a URL
    let is_valid = !form.api_url.trim().is_empty()
        && !form.api_key.trim().is_empty()
        && !form.workspace_slug.trim().is_empty()
        && form.api_url.starts_with("http");

    if is_valid {
        // In a real implementation, you would make an HTTP request to verify connectivity
        render(ToastPartial {
            message: "Plane connection test passed (mock)".to_string(),
            success: true,
        })
    } else {
        render(ToastPartial {
            message: "Plane settings are incomplete or invalid".to_string(),
            success: false,
        })
    }
}
