// SPDX-License-Identifier: MIT OR Apache-2.0
// Hand-written stubs for all proto-generated types used by agileplus-grpc.
//
// These are compiled instead of protoc output when `protoc` is not available.
// All struct fields mirror the proto definitions exactly so that server impls
// compile unchanged.
//
// Traceability: FR-AGP-011

use std::collections::HashMap;

// ── common.proto ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeatureState {
    pub state: String,
    pub next_command: String,
    pub blockers: Vec<String>,
    pub governance: Option<GovernanceStatus>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GovernanceStatus {
    pub all_gates_passed: bool,
    pub total_rules: i32,
    pub passed_rules: i32,
    pub outstanding: Vec<GateViolation>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GateViolation {
    pub fr_id: String,
    pub rule_id: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandResponse {
    pub success: bool,
    pub message: String,
    pub outputs: HashMap<String, String>,
}

// ── core.proto ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feature {
    pub id: i64,
    pub slug: String,
    pub friendly_name: String,
    pub state: String,
    pub target_branch: String,
    pub created_at: String,
    pub updated_at: String,
    pub wp_count: i32,
    pub wp_done: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetFeatureRequest {
    pub slug: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetFeatureResponse {
    pub feature: Option<Feature>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListFeaturesRequest {
    pub state_filter: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListFeaturesResponse {
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetFeatureStateRequest {
    pub slug: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetFeatureStateResponse {
    pub feature_state: Option<FeatureState>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkPackageStatus {
    pub id: i64,
    pub title: String,
    pub state: String,
    pub sequence: i32,
    pub agent_id: String,
    pub pr_url: String,
    pub pr_state: String,
    pub depends_on: Vec<i32>,
    pub file_scope: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListWorkPackagesRequest {
    pub feature_slug: String,
    pub state_filter: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListWorkPackagesResponse {
    pub packages: Vec<WorkPackageStatus>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetWorkPackageStatusRequest {
    pub feature_slug: String,
    pub wp_sequence: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetWorkPackageStatusResponse {
    pub work_package_status: Option<WorkPackageStatus>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheckGovernanceGateRequest {
    pub feature_slug: String,
    pub transition: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheckGovernanceGateResponse {
    pub passed: bool,
    pub violations: Vec<GateViolation>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetAuditTrailRequest {
    pub feature_slug: String,
    pub after_id: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuditEntry {
    pub id: i64,
    pub feature_slug: String,
    pub wp_sequence: i32,
    pub timestamp: String,
    pub actor: String,
    pub transition: String,
    pub evidence_refs: Vec<String>,
    pub prev_hash: Vec<u8>,
    pub hash: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetAuditTrailResponse {
    pub audit_entry: Option<AuditEntry>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VerifyAuditChainRequest {
    pub feature_slug: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VerifyAuditChainResponse {
    pub valid: bool,
    pub entries_verified: i64,
    pub first_invalid_id: String,
    pub error_message: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamAgentEventsRequest {
    pub feature_slug: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentEvent {
    pub event_type: String,
    pub feature_slug: String,
    pub wp_sequence: i32,
    pub agent_id: String,
    pub payload: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamAgentEventsResponse {
    pub event: Option<AgentEvent>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DispatchCommand {
    pub command: String,
    pub feature_slug: String,
    pub args: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DispatchCommandRequest {
    pub command: Option<DispatchCommand>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DispatchCommandResponse {
    pub result: Option<CommandResponse>,
}

// ── integrations.proto ───────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BacklogItem {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub r#type: String,
    pub priority: String,
    pub status: String,
    pub source: String,
    pub feature_slug: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateBacklogItemRequest {
    pub title: String,
    pub description: String,
    pub r#type: String,
    pub priority: String,
    pub source: String,
    pub feature_slug: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateBacklogItemResponse {
    pub item: Option<BacklogItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportBacklogRequest {
    pub items: Vec<CreateBacklogItemRequest>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportBacklogResponse {
    pub items: Vec<BacklogItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetBacklogItemRequest {
    pub backlog_item_id: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GetBacklogItemResponse {
    pub item: Option<BacklogItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListBacklogRequest {
    pub type_filter: String,
    pub state_filter: String,
    pub priority_filter: String,
    pub feature_slug: String,
    pub source_filter: String,
    pub limit: i32,
    pub sort: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListBacklogResponse {
    pub items: Vec<BacklogItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateBacklogStatusRequest {
    pub backlog_item_id: i64,
    pub target_status: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateBacklogStatusResponse {
    pub backlog_item_id: i64,
    pub from_status: String,
    pub to_status: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PopBacklogRequest {
    pub count: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PopBacklogResponse {
    pub items: Vec<BacklogItem>,
}

// ── work_items.proto ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListProjectsRequest {}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectProto {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListProjectsResponse {
    pub projects: Vec<ProjectProto>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListEpicsRequest {
    pub project_id: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EpicProto {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListEpicsResponse {
    pub epics: Vec<EpicProto>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListStoriesRequest {
    pub epic_id: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryProto {
    pub id: i64,
    pub epic_id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub points: i32,
    pub requirement_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListStoriesResponse {
    pub stories: Vec<StoryProto>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncRepositoryRequest {
    pub repo: String,
    pub project_id: i64,
    pub epic_id: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncRepositoryResponse {
    pub stories_synced: i32,
    pub stories_skipped: i32,
    pub errors: Vec<String>,
}

// ── tonic service traits (stubs) ─────────────────────────────────────────────
//
// When protoc is available tonic-build generates these with full Service impls.
// In stub mode we only need the trait definition + a server wrapper that the
// actual service impls can reference.  The wrapper intentionally does NOT
// implement tonic::codegen::Service so that `start_server` (which calls
// .add_service) is naturally unreachable/dead-code in no-protoc builds —
// that function is gated by a #[cfg(not(agileplus_proto_stubs))] attribute in
// server/mod.rs.

use tonic::{Request, Response, Status};

#[tonic::async_trait]
pub trait AgilePlusCoreService: Send + Sync + 'static {
    async fn get_feature(
        &self,
        request: Request<GetFeatureRequest>,
    ) -> Result<Response<GetFeatureResponse>, Status>;

    async fn list_features(
        &self,
        request: Request<ListFeaturesRequest>,
    ) -> Result<Response<ListFeaturesResponse>, Status>;

    async fn get_feature_state(
        &self,
        request: Request<GetFeatureStateRequest>,
    ) -> Result<Response<GetFeatureStateResponse>, Status>;

    async fn list_work_packages(
        &self,
        request: Request<ListWorkPackagesRequest>,
    ) -> Result<Response<ListWorkPackagesResponse>, Status>;

    async fn get_work_package_status(
        &self,
        request: Request<GetWorkPackageStatusRequest>,
    ) -> Result<Response<GetWorkPackageStatusResponse>, Status>;

    async fn check_governance_gate(
        &self,
        request: Request<CheckGovernanceGateRequest>,
    ) -> Result<Response<CheckGovernanceGateResponse>, Status>;

    type GetAuditTrailStream: tokio_stream::Stream<Item = Result<GetAuditTrailResponse, Status>>
        + Send
        + 'static;

    async fn get_audit_trail(
        &self,
        request: Request<GetAuditTrailRequest>,
    ) -> Result<Response<Self::GetAuditTrailStream>, Status>;

    async fn verify_audit_chain(
        &self,
        request: Request<VerifyAuditChainRequest>,
    ) -> Result<Response<VerifyAuditChainResponse>, Status>;

    type StreamAgentEventsStream: tokio_stream::Stream<
            Item = Result<StreamAgentEventsResponse, Status>,
        > + Send
        + 'static;

    async fn stream_agent_events(
        &self,
        request: Request<StreamAgentEventsRequest>,
    ) -> Result<Response<Self::StreamAgentEventsStream>, Status>;

    async fn dispatch_command(
        &self,
        request: Request<DispatchCommandRequest>,
    ) -> Result<Response<DispatchCommandResponse>, Status>;
}

// Thin wrapper — used only so `AgilePlusCoreServiceServer::new(svc)` compiles.
// Does NOT implement tonic::codegen::Service; start_server is unreachable in
// stub mode.
pub struct AgilePlusCoreServiceServer<T>(pub T);

impl<T: AgilePlusCoreService> AgilePlusCoreServiceServer<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T: AgilePlusCoreService> tonic::server::NamedService for AgilePlusCoreServiceServer<T> {
    const NAME: &'static str = "agileplus.v1.AgilePlusCoreService";
}

// ── IntegrationsService ───────────────────────────────────────────────────────

pub struct IntegrationsServiceServer<T>(pub T);

impl<T> IntegrationsServiceServer<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> tonic::server::NamedService for IntegrationsServiceServer<T> {
    const NAME: &'static str = "agileplus.v1.IntegrationsService";
}

// ── WorkItemsService ──────────────────────────────────────────────────────────

#[tonic::async_trait]
pub trait WorkItemsService: Send + Sync + 'static {
    async fn list_projects(
        &self,
        request: Request<ListProjectsRequest>,
    ) -> Result<Response<ListProjectsResponse>, Status>;

    async fn list_epics(
        &self,
        request: Request<ListEpicsRequest>,
    ) -> Result<Response<ListEpicsResponse>, Status>;

    async fn list_stories(
        &self,
        request: Request<ListStoriesRequest>,
    ) -> Result<Response<ListStoriesResponse>, Status>;

    async fn sync_repository(
        &self,
        request: Request<SyncRepositoryRequest>,
    ) -> Result<Response<SyncRepositoryResponse>, Status>;
}

pub struct WorkItemsServiceServer<T>(pub T);

impl<T: WorkItemsService> WorkItemsServiceServer<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T: WorkItemsService> tonic::server::NamedService for WorkItemsServiceServer<T> {
    const NAME: &'static str = "agileplus.v1.WorkItemsService";
}

// Nested server module aliases expected by server/mod.rs and bootstrap.rs imports.
pub mod agile_plus_core_service_server {
    pub use super::{AgilePlusCoreService, AgilePlusCoreServiceServer};
}

pub mod integrations_service_server {
    pub use super::IntegrationsServiceServer;
}

pub mod work_items_service_server {
    pub use super::{WorkItemsService, WorkItemsServiceServer};
}
