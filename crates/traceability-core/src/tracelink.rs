//! `TraceLink` — confidence-scored directed edges in the traceability graph.
//!
//! Source: [`Tracera/crates/tracera-core/src/lib.rs`](https://example.invalid/Tracera/crates/tracera-core/src/lib.rs)
//! (lines 46-72, 184-239, 322-381). Split out of the monolithic `lib.rs` so
//! `artifact` and `matrix` can depend on link types without circular imports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::artifact::{ArtifactKind, ArtifactRef, TraceLinkError};

/// Canonical trace-link relationship vocabulary (ISO 29148 § 5.2.6 + DO-178C Table A-3).
///
/// Source: Tracera `lib.rs:46-57`. All **7** variants preserved verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TraceLinkType {
    /// Requirement is satisfied by a design / implementation artifact.
    Satisfies,
    /// Requirement is verified by a test or evidence artifact.
    Verifies,
    /// Design or code implements a requirement.
    Implements,
    /// Target derives from source (parentage / decomposition).
    DerivesFrom,
    /// Target refines source (more specific version).
    Refines,
    /// Source and target are in conflict.
    ConflictsWith,
    /// Source and target are duplicates of each other.
    Duplicates,
}

impl TraceLinkType {
    /// Returns the SCREAMING_SNAKE string for SQL/Neo4j round-trip.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Satisfies => "SATISFIES",
            Self::Verifies => "VERIFIES",
            Self::Implements => "IMPLEMENTS",
            Self::DerivesFrom => "DERIVES_FROM",
            Self::Refines => "REFINES",
            Self::ConflictsWith => "CONFLICTS_WITH",
            Self::Duplicates => "DUPLICATES",
        }
    }
}

/// Core P0 subset called out in the SOTA research brief.
///
/// Source: Tracera `lib.rs:74-80`.
pub const CORE_TRACE_LINK_TYPES: &[TraceLinkType] = &[
    TraceLinkType::Satisfies,
    TraceLinkType::Verifies,
    TraceLinkType::Implements,
    TraceLinkType::DerivesFrom,
];

/// Returns `true` when `link_type` is one of the P0 core types.
pub fn is_core_link_type(link_type: TraceLinkType) -> bool {
    CORE_TRACE_LINK_TYPES.contains(&link_type)
}

/// A confidence-scored directed edge in the traceability graph.
///
/// Source: Tracera `lib.rs:184-239`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceLink {
    /// Stable graph-internal UUID for this link.
    pub id: Uuid,
    /// Project tenancy.
    pub project_id: Uuid,
    /// Source artifact UUID (join key).
    pub source_artifact_id: Uuid,
    /// Target artifact UUID (join key).
    pub target_artifact_id: Uuid,
    /// Typed source reference (human / external id).
    pub from: ArtifactRef,
    /// Typed target reference (human / external id).
    pub to: ArtifactRef,
    /// Relationship semantics.
    pub link_type: TraceLinkType,
    /// 0.0..=1.0; 1.0 for human-curated links.
    pub confidence: f32,
    /// Optional rationale for the link.
    pub rationale: Option<String>,
    /// Open-ended metadata bag.
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl TraceLink {
    /// Create a new TraceLink, validating that source != target.
    ///
    /// `from` / `to` refs default to UUID-shaped `CodeEntity` placeholders;
    /// callers should overwrite them with typed refs before persistence.
    pub fn new(
        project_id: Uuid,
        source: Uuid,
        target: Uuid,
        link_type: TraceLinkType,
    ) -> Result<Self, TraceLinkError> {
        if source == target {
            return Err(TraceLinkError::SelfLoop);
        }
        Ok(Self {
            id: Uuid::new_v4(),
            project_id,
            source_artifact_id: source,
            target_artifact_id: target,
            from: ArtifactRef::CodeEntity {
                id: source.to_string(),
                lang: "uuid".to_string(),
            },
            to: ArtifactRef::CodeEntity {
                id: target.to_string(),
                lang: "uuid".to_string(),
            },
            link_type,
            confidence: 1.0,
            rationale: None,
            metadata: BTreeMap::new(),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    /// Set confidence, validating the `0.0..=1.0` range.
    pub fn with_confidence(mut self, confidence: f32) -> Result<Self, TraceLinkError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(TraceLinkError::BadConfidence(confidence));
        }
        self.confidence = confidence;
        Ok(self)
    }

    /// True if this link uses one of the P0 SOTA link types.
    pub fn is_core(&self) -> bool {
        is_core_link_type(self.link_type)
    }
}

/// Neo4j relationship labels (one per [`TraceLinkType`]).
///
/// Source: Tracera `lib.rs:322-331`.
pub const NEO4J_RELATIONSHIP_TYPES: &[&str] = &[
    "SATISFIES",
    "VERIFIES",
    "IMPLEMENTS",
    "DERIVES_FROM",
    "REFINES",
    "CONFLICTS_WITH",
    "DUPLICATES",
];

/// Neo4j node labels.
///
/// Source: Tracera `lib.rs:333-344`.
pub const NEO4J_NODE_LABELS: &[&str] = &[
    "Artifact",
    "Requirement",
    "Design",
    "Code",
    "Test",
    "Evidence",
    "Risk",
    "Rationale",
    "Project",
];

/// Declarative Cypher schema for the trace-link graph projection.
///
/// Source: Tracera `lib.rs:346-381`.
pub struct Neo4jSchema;

impl Neo4jSchema {
    /// Uniqueness / existence constraints.
    pub const CONSTRAINTS: &'static [&'static str] = &[
        "CREATE CONSTRAINT artifact_id_unique IF NOT EXISTS FOR (a:Artifact) REQUIRE a.id IS UNIQUE",
        "CREATE CONSTRAINT requirement_id_unique IF NOT EXISTS FOR (r:Requirement) REQUIRE r.id IS UNIQUE",
        "CREATE CONSTRAINT project_id_unique IF NOT EXISTS FOR (p:Project) REQUIRE p.id IS UNIQUE",
    ];

    /// Lookup / range indexes for the common RAG-side queries.
    pub const INDEXES: &'static [&'static str] = &[
        "CREATE INDEX artifact_project_kind IF NOT EXISTS FOR (a:Artifact) ON (a.project_id, a.kind)",
        "CREATE INDEX artifact_external_id IF NOT EXISTS FOR (a:Artifact) ON (a.external_id)",
        "CREATE INDEX requirement_status IF NOT EXISTS FOR (r:Requirement) ON (r.status)",
        "CREATE FULLTEXT INDEX artifact_text IF NOT EXISTS FOR (a:Artifact) ON EACH [a.title, a.description]",
    ];

    /// All DDL statements in apply order (constraints before indexes).
    pub fn all_statements() -> Vec<&'static str> {
        let mut s: Vec<&'static str> = Self::CONSTRAINTS.to_vec();
        s.extend_from_slice(Self::INDEXES);
        s
    }

    /// Return the Neo4j relationship label for a given [`TraceLinkType`].
    pub fn relationship_label_for(link_type: TraceLinkType) -> &'static str {
        link_type.as_db_str()
    }

    /// Return the primary Neo4j node label for a given [`ArtifactKind`].
    pub fn node_label_for(kind: ArtifactKind) -> &'static str {
        kind.neo4j_label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::RequirementId;

    #[test]
    fn trace_link_roundtrips() {
        let link = TraceLink::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            TraceLinkType::Verifies,
        )
        .unwrap();
        assert!(link.is_core());
        let link_type = link.link_type;
        let json = serde_json::to_string(&link).unwrap();
        let parsed: TraceLink = serde_json::from_str(&json).unwrap();
        assert_eq!(link.id, parsed.id);
        assert_eq!(link_type, parsed.link_type);
    }

    #[test]
    fn trace_link_rejects_self_loop() {
        let id = Uuid::new_v4();
        let result = TraceLink::new(Uuid::new_v4(), id, id, TraceLinkType::Satisfies);
        assert!(matches!(result, Err(TraceLinkError::SelfLoop)));
    }

    #[test]
    fn with_confidence_validates_range() {
        let link = TraceLink::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            TraceLinkType::Implements,
        )
        .unwrap();
        assert!(link.clone().with_confidence(0.5).is_ok());
        assert!(matches!(
            link.with_confidence(1.5),
            Err(TraceLinkError::BadConfidence(1.5))
        ));
    }

    #[test]
    fn link_type_db_strings() {
        assert_eq!(TraceLinkType::Satisfies.as_db_str(), "SATISFIES");
        assert_eq!(TraceLinkType::ConflictsWith.as_db_str(), "CONFLICTS_WITH");
        assert_eq!(TraceLinkType::DerivesFrom.as_db_str(), "DERIVES_FROM");
        assert_eq!(TraceLinkType::Duplicates.as_db_str(), "DUPLICATES");
        assert_eq!(TraceLinkType::Refines.as_db_str(), "REFINES");
    }

    #[test]
    fn all_seven_link_types_present() {
        let all = [
            TraceLinkType::Implements,
            TraceLinkType::Verifies,
            TraceLinkType::Duplicates,
            TraceLinkType::Satisfies,
            TraceLinkType::DerivesFrom,
            TraceLinkType::ConflictsWith,
            TraceLinkType::Refines,
        ];
        assert_eq!(all.len(), 7);
        for ty in all {
            assert!(NEO4J_RELATIONSHIP_TYPES.contains(&ty.as_db_str()));
        }
    }

    #[test]
    fn is_core_link_type_matches_const() {
        assert!(is_core_link_type(TraceLinkType::Verifies));
        assert!(!is_core_link_type(TraceLinkType::Duplicates));
        for ty in CORE_TRACE_LINK_TYPES {
            assert!(is_core_link_type(*ty));
        }
    }

    #[test]
    fn neo4j_schema_statements_idempotent() {
        let stmts = Neo4jSchema::all_statements();
        assert!(stmts.len() >= 7);
        for s in &stmts {
            assert!(s.contains("IF NOT EXISTS"), "not idempotent: {s}");
        }
    }

    #[test]
    fn neo4j_labels() {
        assert_eq!(
            Neo4jSchema::node_label_for(ArtifactKind::Requirement),
            "Requirement"
        );
        assert_eq!(
            Neo4jSchema::relationship_label_for(TraceLinkType::Verifies),
            "VERIFIES"
        );
    }

    #[test]
    fn trace_link_typed_refs_roundtrip() {
        let mut link = TraceLink::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            TraceLinkType::Verifies,
        )
        .unwrap();
        link.from = ArtifactRef::Requirement {
            id: RequirementId::from_string("FR-77"),
        };
        link.to = ArtifactRef::Test {
            id: "checkout flow/test verifies receipt".to_string(),
        };
        let json = serde_json::to_string(&link).unwrap();
        let back: TraceLink = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from, link.from);
        assert_eq!(back.to, link.to);
    }
}
