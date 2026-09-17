// SPDX-License-Identifier: MIT OR Apache-2.0
//! Data Transfer Objects for application-layer command/query boundaries.
//!
//! These are plain data structs with no domain logic. They cross the use-case
//! boundary and may be constructed by API handlers, CLI commands, or tests.

// ── Triage / CLI orchestration DTOs (FR-AGP-018, FR-AGP-019, FR-AGP-020) ──────
//
// These are the DTOs that flow through the use-case boundary for the
// CLI triage subcommands (pick, claim, dedup, scan, topology, etc.).
// They reference types from `agileplus-triage` (dedup candidates, claim
// records, repo info).
//
// We re-export the third-party types from this module so callers
// importing `crate::dto::*` get a closed surface.

pub use agileplus_triage::claim::{Claim, ClaimKind, ClaimReason, ClaimState, ClaimStore};
pub use agileplus_triage::dedup::DuplicateCandidate;
pub use agileplus_triage::repo_introspect::RepoInfo;

use serde::{Deserialize, Serialize};

// ── Pick / list ──────────────────────────────────────────────────────────────

/// Request: pick the next N work packages for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickRequest {
    pub agent_id: String,
    pub limit: usize,
    pub lane: Option<String>,
    pub category: Option<String>,
}

/// A picked work package summary returned by `AppState::pick`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickedItem {
    pub wp_id: String,
    pub title: String,
    pub state: String,
    pub dependencies: Vec<String>,
}

// ── Claim lifecycle ─────────────────────────────────────────────────────────

/// Request: claim a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub claim_id: String,
    pub resource: String,
    pub kind: ClaimKind,
    pub agent_id: String,
    pub ttl_seconds: i64,
    /// Structured reason for the claim. Optional on the wire; if
    /// absent, the use case layer falls back to `ClaimReason::default()`
    /// (`Manual("")`).
    #[serde(default)]
    pub reason: ClaimReason,
}

/// Request: refresh a claim's heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub claim_id: String,
}

/// Request: explicitly release a claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRequest {
    pub claim_id: String,
}

/// Request: mark a work package done and release its claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneRequest {
    pub claim_id: String,
    pub wp_id: String,
    pub result: Option<String>,
}

// ── Dedup / scan / topology / export ────────────────────────────────────────

/// Request: find duplicate work-package candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupRequest {
    pub items: Vec<(String, String)>,
    pub threshold: f64,
}

/// Request: scan roots for repo metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRequest {
    pub roots: Vec<String>,
    pub max_depth: Option<usize>,
}

/// Request: produce a topology report from current work-package graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyRequest {
    pub root_wp: Option<String>,
}

/// Request: export current work-package set in the chosen format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub output_path: Option<String>,
    pub with_side: bool,
}

/// Output format selector for `AppState::export`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Markdown,
    Csv,
    Html,
    Mermaid,
    Dot,
}

// ── "Where am I?" snapshot ──────────────────────────────────────────────────

/// Request: snapshot the agent's current work context from a cwd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereRequest {
    pub cwd: String,
}

/// Response: a snapshot of repo, claims, lane/category, and pickable items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereResponse {
    pub repo: Option<RepoInfo>,
    pub active_claims: Vec<Claim>,
    pub lane: Option<String>,
    pub category: Option<String>,
    pub next_pickable: Vec<PickedItem>,
}

// ── Domain DTOs (legacy per-use-case commands) ──────────────────────────────

use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::story::{Story, StoryStatus};

// ── Feature ──────────────────────────────────────────────────────────────────

/// Command: create a new Feature.
#[derive(Debug, Clone)]
pub struct CreateFeatureCmd {
    pub slug: String,
    pub friendly_name: String,
    /// Optional SHA-256 spec hash; defaults to all-zeros if absent.
    pub spec_hash: Option<[u8; 32]>,
    pub target_branch: Option<String>,
}

/// Output of `CreateFeature::execute`.
#[derive(Debug, Clone)]
pub struct FeatureCreatedOutput {
    pub id: i64,
    pub feature: Feature,
}

/// Command: advance a Feature through its lifecycle.
#[derive(Debug, Clone)]
pub struct AdvanceFeatureCmd {
    pub feature_id: i64,
    /// Target state, expressed as the lowercase string (e.g. "specified").
    pub target_state: String,
}

// ── Story ─────────────────────────────────────────────────────────────────────

/// Command: create a new Story under an Epic.
#[derive(Debug, Clone)]
pub struct CreateStoryCmd {
    pub epic_id: i64,
    pub project_id: i64,
    pub title: String,
    pub points: Option<u32>,
}

/// Output of `CreateStory::execute`.
#[derive(Debug, Clone)]
pub struct StoryCreatedOutput {
    pub id: i64,
    pub story: Story,
}

/// Command: transition a Story's status.
#[derive(Debug, Clone)]
pub struct TransitionStoryCmd {
    pub story_id: i64,
    pub target_status: StoryStatus,
}

// ── Epic ──────────────────────────────────────────────────────────────────────

/// Command: create a new Epic.
#[derive(Debug, Clone)]
pub struct CreateEpicCmd {
    pub project_id: i64,
    pub title: String,
}

/// Output of `CreateEpic::execute`.
#[derive(Debug, Clone)]
pub struct EpicCreatedOutput {
    pub id: i64,
}
