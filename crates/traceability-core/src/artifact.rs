//! `Artifact` — the universal node in the traceability graph.
//!
//! Source: [`Tracera/crates/tracera-core/src/lib.rs`](https://example.invalid/Tracera/crates/tracera-core/src/lib.rs)
//! (lines 86-155), with `ArtifactRef` (lines 245-272) and `TraceLinkError` (lines 276-287)
//! split out of the monolithic `lib.rs` into focused submodules.
//!
//! Hybridisation notes (see ADR-0001 §3):
//! * `ArtifactKind` keeps Tracera's 7-variant vocabulary (Requirement/Design/Code/
//!   Test/Evidence/Risk/Rationale). It is the **graph-side** node kind.
//! * AgilePlus' `NodeType` (Intent/Plan/Feature/Story/Task/Spec/Commit/Test/PR/
//!   Bug/Artifact) is the **ontology-side** node kind, kept in `intent_graph`.
//! * The two are mapped at the boundary by [`ArtifactRef::kind_str`]: a `Test`
//!   artifact can be linked from a `Test` intent node and from a `Code`
//!   artifact without loss.
//! * `ArtifactRef` is kept as a tagged enum so Neo4j / SQL can round-trip
//!   each kind. The `kind` discriminant is the database-facing label.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::ids::{NfrId, RequirementId};

/// Role of an [`Artifact`] inside the traceability graph.
///
/// Source: Tracera `lib.rs:87-97`. Vocabulary is **unchanged** in the merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Requirement,
    Design,
    Code,
    Test,
    Evidence,
    Risk,
    Rationale,
}

impl ArtifactKind {
    /// Returns the primary Neo4j node label for this kind.
    pub fn neo4j_label(&self) -> &'static str {
        match self {
            Self::Requirement => "Requirement",
            Self::Design => "Design",
            Self::Code => "Code",
            Self::Test => "Test",
            Self::Evidence => "Evidence",
            Self::Risk => "Risk",
            Self::Rationale => "Rationale",
        }
    }
}

/// Any node in the traceability graph (super-type of [`crate::requirement::Requirement`]).
///
/// Source: Tracera `lib.rs:142-154`. Fields preserved verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Stable graph-internal UUID (v4) used as a join key across stores.
    pub id: Uuid,
    /// Project tenancy.
    pub project_id: Uuid,
    /// Role of this node inside the graph.
    pub kind: ArtifactKind,
    /// Human-readable title.
    pub title: String,
    /// Optional long-form description.
    pub description: Option<String>,
    /// Optional external stable id (e.g. AgilePlus' `wp_id` or `feature_id`,
    /// or a Jira/Linear issue key). Set when the artifact is a foreign key
    /// mirror.
    pub external_id: Option<String>,
    /// Open-ended metadata bag.
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Artifact {
    /// Construct a bare artifact with the given kind and a fresh UUID.
    pub fn new(project_id: Uuid, kind: ArtifactKind, title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            project_id,
            kind,
            title: title.into(),
            description: None,
            external_id: None,
            metadata: BTreeMap::new(),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }
}

/// Foreign-key-shaped reference to any artifact; used on [`crate::tracelink::TraceLink`]
/// so that links can be serialised without their full artifact body.
///
/// Source: Tracera `lib.rs:245-272`. Hybridisation note: the AgilePlus-side
/// `meta.confidence` lives on the link itself, not on the ref, so no merge
/// change is needed here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactRef {
    /// Functional requirement.
    Requirement { id: RequirementId },
    /// Non-functional requirement.
    NonFunctionalRequirement { id: NfrId },
    /// Test (any flavour).
    Test { id: String },
    /// Source-code entity.
    CodeEntity { id: String, lang: String },
    /// User journey.
    Journey { id: String },
    /// Agent run / execution record.
    AgentRun { id: String },
    /// Evidence artifact (sha256 content-addressed).
    Evidence { id: String, sha256: String },
    /// Document reference (path + optional line range).
    Document { id: String, range: Option<String> },
}

impl ArtifactRef {
    /// Lowercase kind string, used as a discriminant in DB / URL routing.
    pub fn kind_str(&self) -> String {
        match self {
            Self::Requirement { .. } => "requirement",
            Self::NonFunctionalRequirement { .. } => "nfr",
            Self::Test { .. } => "test",
            Self::CodeEntity { .. } => "code",
            Self::Journey { .. } => "journey",
            Self::AgentRun { .. } => "agent",
            Self::Evidence { .. } => "evidence",
            Self::Document { .. } => "document",
        }
        .to_string()
    }
}

/// Type alias retained for call-site readability. Source: Tracera `lib.rs:274`.
pub type LinkKind = crate::tracelink::TraceLinkType;

/// Crate-wide error type for construction-time validation of the
/// traceability primitives.
///
/// Source: Tracera `lib.rs:276-287`. New `BadConfidence` variant is kept
/// for forward-compat (Tracera does not use it yet, but `TraceLink` already
/// exposes `confidence: f32`).
#[derive(Debug, thiserror::Error)]
pub enum TraceLinkError {
    /// Source and target artifact ids were equal.
    #[error("TraceLink source_artifact_id and target_artifact_id must differ")]
    SelfLoop,
    /// The artifact's kind did not match the wrapper type's expectation
    /// (e.g. constructing a `Requirement` from a `Code` artifact).
    #[error("Requirement.kind must be REQUIREMENT, got {got:?}")]
    WrongArtifactKind {
        /// The kind the constructor wanted.
        expected: ArtifactKind,
        /// The kind that was actually set.
        got: ArtifactKind,
    },
    /// `confidence` was outside the `0.0..=1.0` range.
    #[error("confidence must be in 0.0..=1.0, got {0}")]
    BadConfidence(f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_new_assigns_kind_and_id() {
        let project = Uuid::new_v4();
        let a = Artifact::new(project, ArtifactKind::Code, "hello");
        assert_eq!(a.project_id, project);
        assert_eq!(a.kind, ArtifactKind::Code);
        assert_eq!(a.title, "hello");
        assert!(a.created_at.is_some());
    }

    #[test]
    fn artifact_kind_neo4j_labels() {
        assert_eq!(ArtifactKind::Requirement.neo4j_label(), "Requirement");
        assert_eq!(ArtifactKind::Code.neo4j_label(), "Code");
        assert_eq!(ArtifactKind::Evidence.neo4j_label(), "Evidence");
    }

    #[test]
    fn artifact_ref_kind_str() {
        let r = ArtifactRef::Requirement {
            id: RequirementId::from_string("FR-1"),
        };
        assert_eq!(r.kind_str(), "requirement");
        let r = ArtifactRef::CodeEntity {
            id: "mod::fn".to_string(),
            lang: "rust".to_string(),
        };
        assert_eq!(r.kind_str(), "code");
        let r = ArtifactRef::Evidence {
            id: "ev-1".to_string(),
            sha256: "0".repeat(64),
        };
        assert_eq!(r.kind_str(), "evidence");
    }

    #[test]
    fn artifact_ref_serde_tagged_roundtrip() {
        let r = ArtifactRef::Test {
            id: "T-1".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"test\""));
        let back: ArtifactRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn trace_link_error_messages_are_stable() {
        let e = TraceLinkError::SelfLoop;
        assert_eq!(
            e.to_string(),
            "TraceLink source_artifact_id and target_artifact_id must differ"
        );
        let e = TraceLinkError::WrongArtifactKind {
            expected: ArtifactKind::Requirement,
            got: ArtifactKind::Code,
        };
        assert!(e.to_string().contains("REQUIREMENT"));
        let e = TraceLinkError::BadConfidence(1.5);
        assert!(e.to_string().contains("1.5"));
    }
}
