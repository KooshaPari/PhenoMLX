// SPDX-License-Identifier: MIT OR Apache-2.0
//! API response types — stable JSON shapes for all endpoints.
//!
//! These types are separate from domain entities so internal representations
//! can change without breaking the public API contract.
//!
//! Traceability: WP15-T086

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use agileplus_domain::domain::audit::AuditEntry;
use agileplus_domain::domain::epic::Epic;
use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::governance::GovernanceContract;
use agileplus_domain::domain::project::Project;
use agileplus_domain::domain::story::Story;
use agileplus_domain::domain::user::User;
use agileplus_domain::domain::work_package::WorkPackage;

// ----- Features -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeatureResponse {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub state: String,
    pub target_branch: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Feature> for FeatureResponse {
    fn from(f: Feature) -> Self {
        Self {
            id: f.id,
            slug: f.slug,
            name: f.friendly_name,
            state: format!("{:?}", f.state).to_lowercase(),
            target_branch: f.target_branch,
            created_at: f.created_at.to_rfc3339(),
            updated_at: f.updated_at.to_rfc3339(),
        }
    }
}

// ----- Work Packages -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkPackageResponse {
    pub id: i64,
    pub feature_id: i64,
    pub title: String,
    pub state: String,
    pub sequence: i32,
    pub acceptance_criteria: String,
    pub pr_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<WorkPackage> for WorkPackageResponse {
    fn from(wp: WorkPackage) -> Self {
        Self {
            id: wp.id,
            feature_id: wp.feature_id,
            title: wp.title,
            state: format!("{:?}", wp.state).to_lowercase(),
            sequence: wp.sequence,
            acceptance_criteria: wp.acceptance_criteria,
            pr_url: wp.pr_url,
            created_at: wp.created_at.to_rfc3339(),
            updated_at: wp.updated_at.to_rfc3339(),
        }
    }
}

// ----- Governance -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GovernanceResponse {
    pub id: i64,
    pub feature_id: i64,
    pub version: i32,
    pub rules_count: usize,
    pub bound_at: String,
}

impl From<GovernanceContract> for GovernanceResponse {
    fn from(c: GovernanceContract) -> Self {
        Self {
            id: c.id,
            feature_id: c.feature_id,
            version: c.version,
            rules_count: c.rules.len(),
            bound_at: c.bound_at.to_rfc3339(),
        }
    }
}

// ----- Audit -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuditEntryResponse {
    pub id: i64,
    pub feature_id: i64,
    pub wp_id: Option<i64>,
    pub timestamp: String,
    pub actor: String,
    pub transition: String,
    pub hash: String,
}

impl From<AuditEntry> for AuditEntryResponse {
    fn from(e: AuditEntry) -> Self {
        Self {
            id: e.id,
            feature_id: e.feature_id,
            wp_id: e.wp_id,
            timestamp: e.timestamp.to_rfc3339(),
            actor: e.actor,
            transition: e.transition,
            hash: e.hash.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }
}

// ----- Projects -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProjectResponse {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            slug: p.slug,
            name: p.name,
            description: p.description,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

// ----- Epics -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EpicResponse {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub owner_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Epic> for EpicResponse {
    fn from(e: Epic) -> Self {
        Self {
            id: e.id,
            project_id: e.project_id,
            title: e.title,
            description: e.description,
            status: e.status.to_string(),
            owner_id: e.owner_id,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

// ----- Stories -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StoryResponse {
    pub id: i64,
    pub epic_id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub points: Option<u32>,
    pub assignee_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Story> for StoryResponse {
    fn from(s: Story) -> Self {
        Self {
            id: s.id,
            epic_id: s.epic_id,
            project_id: s.project_id,
            title: s.title,
            description: s.description,
            status: s.status.to_string(),
            points: s.points,
            assignee_id: s.assignee_id,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

// ----- Users -----

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: i64,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub avatar_url: Option<String>,
    pub github_login: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            display_name: u.display_name,
            email: u.email,
            role: u.role.to_string(),
            status: u.status.to_string(),
            avatar_url: u.avatar_url,
            github_login: u.github_login,
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
        }
    }
}

// ----- Health -----

#[derive(Debug, Serialize)]
pub struct SimpleHealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

impl SimpleHealthResponse {
    pub fn ok() -> Self {
        Self {
            status: "ok",
            service: "agileplus-api",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub fn healthy() -> Self {
        Self {
            status: "healthy",
            service: "agileplus-api",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

impl HealthResponse {
    pub fn ok() -> Self {
        Self {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

// ----- Detailed Health (T070) -----

#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ServiceHealth {
    pub fn healthy(latency_ms: u64) -> Self {
        Self {
            status: "healthy".to_owned(),
            latency_ms: Some(latency_ms),
            error: None,
        }
    }

    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            status: "degraded".to_owned(),
            latency_ms: None,
            error: Some(reason.into()),
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: "unavailable".to_owned(),
            latency_ms: None,
            error: Some(reason.into()),
        }
    }

    pub fn not_configured() -> Self {
        Self {
            status: "not_configured".to_string(),
            latency_ms: None,
            error: Some("not configured in this deployment".to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiHealth {
    pub status: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct DetailedHealthResponse {
    /// Overall status: "healthy" | "degraded" | "unavailable"
    pub status: String,
    pub timestamp: String,
    pub services: std::collections::HashMap<String, ServiceHealth>,
    pub api: ApiHealth,
}

impl DetailedHealthResponse {
    /// Build a basic healthy response (used when no external services are wired up).
    pub fn basic(uptime_seconds: u64) -> Self {
        use std::collections::HashMap;
        let services: HashMap<String, ServiceHealth> = [("sqlite", ServiceHealth::healthy(0))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        Self {
            status: "healthy".to_owned(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            services,
            api: ApiHealth {
                status: "healthy".to_owned(),
                uptime_seconds,
            },
        }
    }

    /// Derive overall status from individual service statuses.
    pub fn compute_status(
        services: &std::collections::HashMap<String, ServiceHealth>,
    ) -> &'static str {
        // Only consider services that are actually configured.
        let configured: Vec<_> = services
            .values()
            .filter(|s| s.status != "not_configured")
            .collect();
        if configured.iter().any(|s| s.status == "unavailable") {
            return "unavailable";
        }
        if configured.iter().any(|s| s.status == "degraded") {
            return "degraded";
        }
        "healthy"
    }
}
