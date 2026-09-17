//! OpenAPI specification generator (MVP).
//!
//! Traceability: #118 (OpenAPI governance), #138 (scaffold retry).
//!
//! Scope: 5 representative endpoints spanning features, work packages,
//! events, audit, and governance. A follow-up PR will annotate the
//! remaining 31 handlers to reach full coverage.
//!
//! # Usage
//!
//! ```no_run
//! use agileplus_api::openapi::ApiDoc;
//! use utoipa::OpenApi;
//! // utoipa 5 removed `OpenApi::to_yaml()`; serialize via serde_yaml.
//! let yaml = serde_yaml::to_string(&ApiDoc::openapi()).unwrap();
//! ```
//!
//! A `--dump-openapi` flag on `agileplus-api`'s binary writes this to
//! `openapi.yaml` at the workspace root. The CI drift check that compares
//! the committed YAML to a freshly dumped copy is **deferred** until the
//! `agileplus-plugin-core` git dependency (SHA `ba652dec…`) resolves
//! — the workspace currently cannot `cargo build` due to that 404.

use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};

use crate::responses::{
    AuditEntryResponse, FeatureResponse, GovernanceResponse, WorkPackageResponse,
};
use crate::routes::events::EventResponse;
use crate::routes::features::CreateFeatureRequest;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "AgilePlus REST API",
        version = "0.2.1",
        description = "MVP OpenAPI scaffold — 5 representative endpoints. \
                       Full coverage of all 36 handlers is tracked as a follow-up.",
        license(name = "MIT"),
    ),
    paths(
        crate::routes::features::list_features,
        crate::routes::features::create_feature,
        crate::routes::work_packages::get_work_package,
        crate::routes::events::list_events,
        crate::routes::audit::get_audit_trail,
        crate::routes::governance::get_governance,
    ),
    components(
        schemas(
            FeatureResponse,
            CreateFeatureRequest,
            WorkPackageResponse,
            EventResponse,
            AuditEntryResponse,
            GovernanceResponse,
        ),
    ),
    tags(
        (name = "features", description = "Feature CRUD + state transitions"),
        (name = "work-packages", description = "Work package CRUD + state transitions"),
        (name = "events", description = "Domain event queries"),
        (name = "audit", description = "Audit trail inspection + verification"),
        (name = "governance", description = "Governance contracts + validation"),
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
            );
        }
    }
}
