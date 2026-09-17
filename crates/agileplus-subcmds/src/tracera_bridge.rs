// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bridge between AgilePlus sub-commands and the [`tracera-core`] model.
//!
//! AgilePlus sub-commands need to create, serialize, and reason about
//! trace-links (Requirement ↔ Test, NFR ↔ Code, …) when surfacing
//! traceability information back to the user (e.g. from the
//! `agileplus-trace-validator` and `agileplus-triage` crates). This module
//! is the single integration point that re-exports the
//! `tracera-core` entity types and provides a small set of opinionated
//! constructors and helpers tuned for the AgilePlus CLI surface.
//!
//! # Type re-exports
//!
//! * [`FrId`]   — re-export of `tracera_core::RequirementId` (the `FR-…`
//!   functional-requirement identifier; named `FrId` here to match the
//!   AgilePlus vocabulary).
//! * [`NfrId`]  — re-export of `tracera_core::NfrId` (the `NFR-…`
//!   non-functional-requirement identifier).
//! * [`TraceLink`] — newtype wrapper around `tracera_core::TraceLink` that
//!   adds AgilePlus-flavoured constructors and JSON helpers used by the
//!   `agileplus platform` and `agileplus dashboard` sub-commands.
//!
//! Traceability: T-9 (manager roll-up, 2026-06-12).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Re-exports of tracera-core id types
// ---------------------------------------------------------------------------

/// AgilePlus alias for `tracera_core::RequirementId` — the `FR-…`
/// functional-requirement identifier.
pub use tracera_core::ids::RequirementId as FrId;

/// AgilePlus alias for `tracera_core::NfrId` — the `NFR-…`
/// non-functional-requirement identifier.
pub use tracera_core::ids::NfrId;

// ---------------------------------------------------------------------------
// Bridge errors
// ---------------------------------------------------------------------------

/// Errors raised by [`TraceLink`] helpers in this bridge.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// Source and target artifact ids must differ.
    #[error("TraceLink source and target must differ (got {0})")]
    SelfLoop(Uuid),
    /// `confidence` must be in the `0.0..=1.0` range.
    #[error("confidence must be in 0.0..=1.0, got {0}")]
    BadConfidence(f32),
}

impl From<tracera_core::TraceLinkError> for BridgeError {
    fn from(err: tracera_core::TraceLinkError) -> Self {
        match err {
            tracera_core::TraceLinkError::SelfLoop => {
                // The caller passed the same id twice; we don't have it here,
                // so re-emit a nil uuid (the inner field is for context only).
                BridgeError::SelfLoop(Uuid::nil())
            }
            tracera_core::TraceLinkError::BadConfidence(c) => BridgeError::BadConfidence(c),
            // WrongArtifactKind only happens at the Requirement::new boundary
            // and is unreachable from the bridge's public API; map it to
            // SelfLoop as a defensive default.
            tracera_core::TraceLinkError::WrongArtifactKind { .. } => {
                BridgeError::SelfLoop(Uuid::nil())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AgilePlus-flavored TraceLink wrapper
// ---------------------------------------------------------------------------

/// A confidence-scored directed edge in the AgilePlus traceability graph.
///
/// Newtype around [`tracera_core::TraceLink`] that exposes AgilePlus-flavoured
/// constructors and JSON helpers, and pins the `link_type` set to the P0
/// core vocabulary (Satisfies / Verifies / Implements / DerivesFrom).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceLink(tracera_core::TraceLink);

impl TraceLink {
    /// Create a new AgilePlus trace-link.
    ///
    /// Validates that `source != target` and that `confidence` is in the
    /// `0.0..=1.0` range. Defaults `confidence` to `1.0` (human-curated)
    /// when `None` is passed.
    pub fn new(
        project_id: Uuid,
        source: Uuid,
        target: Uuid,
        link_type: tracera_core::TraceLinkType,
        confidence: Option<f32>,
    ) -> Result<Self, BridgeError> {
        if source == target {
            return Err(BridgeError::SelfLoop(source));
        }
        let confidence = confidence.unwrap_or(1.0);
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(BridgeError::BadConfidence(confidence));
        }

        // Build the inner TraceLink and then patch confidence / metadata.
        let mut inner =
            tracera_core::TraceLink::new(project_id, source, target, link_type)
                .map_err(BridgeError::from)?;
        inner.confidence = confidence;
        inner.rationale = Some("created via agileplus-subcmds tracera_bridge".to_string());
        Ok(Self(inner))
    }

    /// Create a [`TraceLink`] whose `from` side is a functional requirement
    /// (FR) and whose `to` side is a generic artifact id (e.g. a test or
    /// code-entity). Convenience for the `agileplus-trace-validator` flow.
    ///
    /// `source_artifact_id` is the UUID form of the FR — the inner
    /// `TraceLink` always stores `source_artifact_id`/`target_artifact_id`
    /// as UUIDs even when the typed `from` ref is `ArtifactRef::Requirement`.
    pub fn from_requirement_to(
        project_id: Uuid,
        source_artifact_id: Uuid,
        from_fr: &FrId,
        target_artifact_id: Uuid,
        to_ref: tracera_core::ArtifactRef,
        link_type: tracera_core::TraceLinkType,
    ) -> Result<Self, BridgeError> {
        let mut link = Self::new(
            project_id,
            source_artifact_id,
            target_artifact_id,
            link_type,
            None,
        )?;
        link.0.from = tracera_core::ArtifactRef::Requirement { id: from_fr.clone() };
        link.0.to = to_ref;
        Ok(link)
    }

    /// Borrow the inner `tracera_core::TraceLink`.
    pub fn inner(&self) -> &tracera_core::TraceLink {
        &self.0
    }

    /// Consume the wrapper and return the inner `tracera_core::TraceLink`.
    pub fn into_inner(self) -> tracera_core::TraceLink {
        self.0
    }

    /// True if this link uses one of the P0 SOTA link types.
    pub fn is_core(&self) -> bool {
        self.0.is_core()
    }

    /// True if the link has human-curated confidence (== 1.0).
    pub fn is_human_curated(&self) -> bool {
        (self.0.confidence - 1.0).abs() < f32::EPSILON
    }

    /// Render the link as a one-line JSON object suitable for inclusion in
    /// the AgilePlus CLI's `--format=json` output.
    pub fn to_jsonl(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.0)
    }

    /// Return the source artifact id (UUID form, even if `from` is a
    /// typed `ArtifactRef` like `Requirement { id: FrId }`).
    pub fn source_artifact_id(&self) -> Uuid {
        self.0.source_artifact_id
    }

    /// Return the target artifact id.
    pub fn target_artifact_id(&self) -> Uuid {
        self.0.target_artifact_id
    }

    /// Return the link type.
    pub fn link_type(&self) -> tracera_core::TraceLinkType {
        self.0.link_type
    }

    /// Attach an AgilePlus-side metadata key/value pair (e.g. the worklog
    /// id that produced the link).
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.0.metadata.insert(key.into(), value);
        self
    }

    /// Borrow the AgilePlus-side metadata map.
    pub fn metadata(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.0.metadata
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tracera_core::{ArtifactRef, TraceLinkType};

    #[test]
    fn new_trace_link_validates_self_loop() {
        let project = Uuid::new_v4();
        let id = Uuid::new_v4();
        let err = TraceLink::new(project, id, id, TraceLinkType::Satisfies, None).unwrap_err();
        match err {
            BridgeError::SelfLoop(returned) => assert_eq!(returned, id),
            other => panic!("expected SelfLoop, got {other:?}"),
        }
    }

    #[test]
    fn new_trace_link_validates_confidence_range() {
        let project = Uuid::new_v4();
        let err = TraceLink::new(
            project,
            Uuid::new_v4(),
            Uuid::new_v4(),
            TraceLinkType::Verifies,
            Some(1.5),
        )
        .unwrap_err();
        assert!(matches!(err, BridgeError::BadConfidence(c) if (c - 1.5).abs() < f32::EPSILON));

        let err2 = TraceLink::new(
            project,
            Uuid::new_v4(),
            Uuid::new_v4(),
            TraceLinkType::Verifies,
            Some(f32::NAN),
        )
        .unwrap_err();
        assert!(matches!(err2, BridgeError::BadConfidence(_)));
    }

    #[test]
    fn from_requirement_to_attaches_typed_from_ref() {
        let project = Uuid::new_v4();
        let fr = FrId::from_string("FR-048");
        let source_artifact_id = Uuid::new_v4();
        let to_artifact = Uuid::new_v4();
        let to_ref = ArtifactRef::Test {
            id: "checkout flow/test verifies receipt".to_string(),
        };

        let link = TraceLink::from_requirement_to(
            project,
            source_artifact_id,
            &fr,
            to_artifact,
            to_ref.clone(),
            TraceLinkType::Verifies,
        )
        .expect("valid FR→Test link should construct cleanly");

        // The inner from-ref should be a typed Requirement variant.
        match &link.inner().from {
            ArtifactRef::Requirement { id } => assert_eq!(id.as_str(), fr.as_str()),
            other => panic!("expected Requirement from-ref, got {other:?}"),
        }
        match &link.inner().to {
            ArtifactRef::Test { id } => {
                assert_eq!(id, "checkout flow/test verifies receipt")
            }
            other => panic!("expected Test to-ref, got {other:?}"),
        }

        assert_eq!(link.source_artifact_id(), source_artifact_id);
        assert_eq!(link.target_artifact_id(), to_artifact);
        assert!(link.is_core(), "Verifies is a P0 link type");
        assert!(link.is_human_curated(), "default confidence is 1.0");
        assert_eq!(link.link_type(), TraceLinkType::Verifies);

        // JSON round-trip should succeed and preserve the link type.
        let json = link.to_jsonl().expect("jsonl serialize");
        assert!(json.contains("\"VERIFIES\""), "jsonl missing link type: {json}");
    }

    #[test]
    fn with_metadata_round_trips() {
        let project = Uuid::new_v4();
        let link = TraceLink::new(
            project,
            Uuid::new_v4(),
            Uuid::new_v4(),
            TraceLinkType::Implements,
            Some(0.42),
        )
        .unwrap()
        .with_metadata("worklog_id", serde_json::json!("wl-2026-06-12-001"));

        assert_eq!(
            link.metadata().get("worklog_id").unwrap(),
            &serde_json::json!("wl-2026-06-12-001")
        );
        assert!(!link.is_human_curated());
    }
}
