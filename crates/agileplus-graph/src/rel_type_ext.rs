// SPDX-License-Identifier: MIT OR Apache-2.0
//! Extended relationship types for multi-agent workflows.
//!
//! This module provides the [`is_multi_agent`] helper and re-exports
//! [`RelType`] so consumers can use the extended set without importing
//! from `types.rs` directly.
//!
//! Traceability: audit rec #24 (graph relationship type extensions).

pub use crate::types::RelType;

/// Returns `true` for relationship types that represent cross-agent
/// interactions (ownership, dispatch, verification, and retry chains).
pub fn is_multi_agent(rel: &RelType) -> bool {
    matches!(
        rel,
        RelType::OwnsClaim
            | RelType::ClaimsWorktree
            | RelType::DispatchedBy
            | RelType::Verifies
            | RelType::Produces
            | RelType::Consumes
            | RelType::Retries
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_multi_agent_true_for_new_variants() {
        assert!(is_multi_agent(&RelType::OwnsClaim));
        assert!(is_multi_agent(&RelType::ClaimsWorktree));
        assert!(is_multi_agent(&RelType::DispatchedBy));
        assert!(is_multi_agent(&RelType::Verifies));
        assert!(is_multi_agent(&RelType::Produces));
        assert!(is_multi_agent(&RelType::Consumes));
        assert!(is_multi_agent(&RelType::Retries));
    }

    #[test]
    fn is_multi_agent_false_for_original_variants() {
        assert!(!is_multi_agent(&RelType::Owns));
        assert!(!is_multi_agent(&RelType::AssignedTo));
        assert!(!is_multi_agent(&RelType::DependsOn));
        assert!(!is_multi_agent(&RelType::Blocks));
        assert!(!is_multi_agent(&RelType::Tagged));
        assert!(!is_multi_agent(&RelType::InProject));
    }

    #[test]
    fn rel_type_as_str_includes_new_variants() {
        assert_eq!(RelType::OwnsClaim.as_str(), "OWNS_CLAIM");
        assert_eq!(RelType::ClaimsWorktree.as_str(), "CLAIMS_WORKTREE");
        assert_eq!(RelType::DispatchedBy.as_str(), "DISPATCHED_BY");
        assert_eq!(RelType::Verifies.as_str(), "VERIFIES");
        assert_eq!(RelType::Produces.as_str(), "PRODUCES");
        assert_eq!(RelType::Consumes.as_str(), "CONSUMES");
        assert_eq!(RelType::Retries.as_str(), "RETRIES");
    }

    #[test]
    fn rel_type_round_trip_via_serde() {
        for rt in [
            RelType::Owns,
            RelType::AssignedTo,
            RelType::DependsOn,
            RelType::Blocks,
            RelType::Tagged,
            RelType::InProject,
            RelType::OwnsClaim,
            RelType::ClaimsWorktree,
            RelType::DispatchedBy,
            RelType::Verifies,
            RelType::Produces,
            RelType::Consumes,
            RelType::Retries,
        ] {
            let json = serde_json::to_string(&rt).unwrap();
            let back: RelType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, rt, "serde round-trip failed for {rt:?}");
        }
    }

    #[test]
    fn rel_type_old_json_still_deserializes() {
        // Simulate JSON from an old client that only knows the 6 variants.
        let old_json = r#"["Owns","AssignedTo","DependsOn","Blocks","Tagged","InProject"]"#;
        let parsed: Vec<RelType> = serde_json::from_str(old_json).unwrap();
        assert_eq!(parsed.len(), 6);
        assert_eq!(parsed[0], RelType::Owns);
        assert_eq!(parsed[5], RelType::InProject);
    }
}
