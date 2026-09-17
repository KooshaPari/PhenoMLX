//! Integration tests for AcceptanceContract + TraceRef cross-domain linkage.
//!
//! This test suite validates that acceptance criteria (from WorkPackage/Feature acceptance)
//! can be linked to traced artifacts in Tracera, forming a bidirectional traceability bridge.

use agileplus_domain::adapters::noop_trace_adapter::NoopTraceAdapter;
use agileplus_domain::ports::traceability_port::TraceabilityPort;
use agileplus_domain::traceability::TraceRef;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn make_trace_ref(trace_id: &str, artifact_type: &str) -> TraceRef {
    TraceRef {
        trace_id: trace_id.to_string(),
        artifact_type: artifact_type.to_string(),
        linked_at: Utc::now(),
    }
}

/// An acceptance criterion — a single, verifiable condition that must be met for a feature/story to be done.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptanceCriterion {
    /// Unique ID for this criterion (e.g. AC-1, AC-2).
    pub id: String,
    /// Human-readable criterion description (e.g. "User can enter valid email").
    pub description: String,
    /// Whether this criterion has been verified as met.
    pub verified: bool,
    /// Optional link to a traced artifact (requirement, evidence, test case).
    pub trace_ref: Option<TraceRef>,
}

impl AcceptanceCriterion {
    /// Construct a new unverified criterion with no trace link.
    pub fn new(id: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            description: description.to_string(),
            verified: false,
            trace_ref: None,
        }
    }

    /// Link this criterion to a traced artifact.
    pub fn with_trace(mut self, trace_ref: TraceRef) -> Self {
        self.trace_ref = Some(trace_ref);
        self
    }

    /// Mark this criterion as verified.
    pub fn mark_verified(mut self) -> Self {
        self.verified = true;
        self
    }
}

/// An acceptance contract — a collection of criteria that must be met for a work item to be considered done.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceContract {
    /// Entity (Feature, Story, WorkPackage) that this contract governs.
    pub entity_id: Uuid,
    /// The set of acceptance criteria.
    pub criteria: Vec<AcceptanceCriterion>,
}

impl AcceptanceContract {
    /// Create a new, empty acceptance contract for an entity.
    pub fn new(entity_id: Uuid) -> Self {
        Self {
            entity_id,
            criteria: Vec::new(),
        }
    }

    /// Add a criterion to the contract.
    pub fn add_criterion(mut self, criterion: AcceptanceCriterion) -> Self {
        self.criteria.push(criterion);
        self
    }

    /// Count verified criteria.
    pub fn verified_count(&self) -> usize {
        self.criteria.iter().filter(|c| c.verified).count()
    }

    /// Check if all criteria are verified.
    pub fn all_verified(&self) -> bool {
        !self.criteria.is_empty() && self.criteria.iter().all(|c| c.verified)
    }

    /// Retrieve all trace references in this contract.
    pub fn trace_refs(&self) -> Vec<TraceRef> {
        self.criteria
            .iter()
            .filter_map(|c| c.trace_ref.clone())
            .collect()
    }

    /// Verify a criterion by its ID.
    pub fn verify_criterion(&mut self, criterion_id: &str) -> Result<(), String> {
        self.criteria
            .iter_mut()
            .find(|c| c.id == criterion_id)
            .map(|c| {
                c.verified = true;
            })
            .ok_or_else(|| format!("Criterion not found: {}", criterion_id))
    }

    /// Link a criterion to a traced artifact. Returns error if criterion not found.
    pub fn link_criterion_to_trace(
        &mut self,
        criterion_id: &str,
        trace_ref: TraceRef,
    ) -> Result<(), String> {
        self.criteria
            .iter_mut()
            .find(|c| c.id == criterion_id)
            .map(|c| {
                c.trace_ref = Some(trace_ref);
            })
            .ok_or_else(|| format!("Criterion not found: {}", criterion_id))
    }

    /// Get a criterion by ID.
    pub fn get_criterion(&self, criterion_id: &str) -> Option<&AcceptanceCriterion> {
        self.criteria.iter().find(|c| c.id == criterion_id)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[test]
fn test_acceptance_contract_links_to_trace_ref() {
    let trace_id = "FR-100".to_string();
    let trace_ref = make_trace_ref(&trace_id, "requirement");

    let criterion = AcceptanceCriterion::new("AC-1", "User can log in with email")
        .with_trace(trace_ref.clone());

    assert_eq!(criterion.trace_ref.as_ref().unwrap().trace_id, trace_id);
    assert_eq!(
        criterion.trace_ref.as_ref().unwrap().artifact_type,
        "requirement"
    );
}

#[test]
fn test_multiple_criteria_each_with_different_trace_refs() {
    let entity_id = Uuid::new_v4();

    let trace_ref_1 = make_trace_ref("FR-101", "requirement");
    let trace_ref_2 = make_trace_ref("TEST-42", "test_case");
    let trace_ref_3 = make_trace_ref("EV-7", "evidence");

    let contract = AcceptanceContract::new(entity_id)
        .add_criterion(
            AcceptanceCriterion::new("AC-1", "Valid email format check")
                .with_trace(trace_ref_1),
        )
        .add_criterion(
            AcceptanceCriterion::new("AC-2", "Password strength validation")
                .with_trace(trace_ref_2),
        )
        .add_criterion(
            AcceptanceCriterion::new("AC-3", "Session token issued after login")
                .with_trace(trace_ref_3),
        );

    assert_eq!(contract.criteria.len(), 3);

    let refs = contract.trace_refs();
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[0].artifact_type, "requirement");
    assert_eq!(refs[1].artifact_type, "test_case");
    assert_eq!(refs[2].artifact_type, "evidence");
}

#[test]
fn test_acceptance_contract_serializes_with_trace_refs() {
    let entity_id = Uuid::new_v4();
    let trace_ref = make_trace_ref("FR-200", "specification");

    let contract = AcceptanceContract::new(entity_id).add_criterion(
        AcceptanceCriterion::new("AC-1", "Form validation works")
            .with_trace(trace_ref.clone()),
    );

    // Serialize to JSON
    let json = serde_json::to_string(&contract).expect("serialize contract");

    // Deserialize from JSON
    let deserialized: AcceptanceContract =
        serde_json::from_str(&json).expect("deserialize contract");

    // Verify structure
    assert_eq!(deserialized.entity_id, entity_id);
    assert_eq!(deserialized.criteria.len(), 1);
    assert_eq!(
        deserialized.criteria[0].trace_ref.as_ref().unwrap().trace_id,
        "FR-200"
    );
    assert_eq!(
        deserialized.criteria[0].trace_ref.as_ref().unwrap().artifact_type,
        "specification"
    );
}

#[test]
fn test_unverified_criterion_with_trace_ref_not_counted_as_done() {
    let entity_id = Uuid::new_v4();
    let trace_ref = make_trace_ref("FR-300", "requirement");

    let contract = AcceptanceContract::new(entity_id)
        .add_criterion(
            AcceptanceCriterion::new("AC-1", "Email validation").with_trace(trace_ref),
        )
        .add_criterion(AcceptanceCriterion::new("AC-2", "Password validation"));

    // Neither criterion is verified yet
    assert_eq!(contract.verified_count(), 0);
    assert!(!contract.all_verified());

    // Even though AC-1 has a trace ref, it's not verified
    assert!(contract.get_criterion("AC-1").unwrap().trace_ref.is_some());
    assert!(!contract.get_criterion("AC-1").unwrap().verified);
}

#[test]
fn test_trace_ref_artifact_type_preserved_in_contract() {
    let entity_id = Uuid::new_v4();

    let artifact_types = vec![
        "requirement",
        "test_case",
        "evidence",
        "specification",
        "acceptance_test",
    ];

    let mut contract = AcceptanceContract::new(entity_id);

    for (idx, artifact_type) in artifact_types.iter().enumerate() {
        let trace_ref = make_trace_ref(&format!("ART-{}", idx), artifact_type);
        let criterion = AcceptanceCriterion::new(&format!("AC-{}", idx), "Test criterion")
            .with_trace(trace_ref);
        contract = contract.add_criterion(criterion);
    }

    // Verify each artifact type is preserved
    for (idx, expected_type) in artifact_types.iter().enumerate() {
        let criterion = contract.get_criterion(&format!("AC-{}", idx)).unwrap();
        let actual_type = &criterion.trace_ref.as_ref().unwrap().artifact_type;
        assert_eq!(actual_type, expected_type);
    }
}

#[test]
fn test_acceptance_contract_round_trip_serialization() {
    let entity_id = Uuid::new_v4();

    // Build a contract with mixed verified/unverified and with/without trace refs
    let mut contract = AcceptanceContract::new(entity_id)
        .add_criterion(
            AcceptanceCriterion::new("AC-1", "With trace, unverified").with_trace(make_trace_ref("FR-400", "requirement")),
        )
        .add_criterion(AcceptanceCriterion::new(
            "AC-2",
            "Without trace, unverified",
        ))
        .add_criterion(
            AcceptanceCriterion::new("AC-3", "With trace, verified").with_trace(make_trace_ref("TEST-50", "test_case")),
        );

    // Verify AC-3
    contract.verify_criterion("AC-3").expect("verify");

    // Serialize and deserialize
    let json = serde_json::to_string(&contract).expect("serialize");
    let deserialized: AcceptanceContract = serde_json::from_str(&json).expect("deserialize");

    // Verify all properties are preserved
    assert_eq!(deserialized.entity_id, entity_id);
    assert_eq!(deserialized.criteria.len(), 3);

    let ac1 = deserialized.get_criterion("AC-1").unwrap();
    assert_eq!(ac1.description, "With trace, unverified");
    assert!(!ac1.verified);
    assert!(ac1.trace_ref.is_some());
    assert_eq!(ac1.trace_ref.as_ref().unwrap().trace_id, "FR-400");

    let ac2 = deserialized.get_criterion("AC-2").unwrap();
    assert_eq!(ac2.description, "Without trace, unverified");
    assert!(!ac2.verified);
    assert!(ac2.trace_ref.is_none());

    let ac3 = deserialized.get_criterion("AC-3").unwrap();
    assert_eq!(ac3.description, "With trace, verified");
    assert!(ac3.verified);
    assert!(ac3.trace_ref.is_some());
    assert_eq!(ac3.trace_ref.as_ref().unwrap().trace_id, "TEST-50");
}

#[test]
fn test_link_nonexistent_criterion_returns_error() {
    let entity_id = Uuid::new_v4();
    let mut contract = AcceptanceContract::new(entity_id);

    let trace_ref = make_trace_ref("FR-500", "requirement");

    // Try to link to a criterion that doesn't exist
    let result = contract.link_criterion_to_trace("AC-NONEXISTENT", trace_ref);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Criterion not found"));
}

#[tokio::test]
async fn test_verify_criterion_with_linked_trace() {
    let entity_id = Uuid::new_v4();
    let adapter = NoopTraceAdapter;

    let trace_ref = make_trace_ref("FR-600", "requirement");

    // Simulate linking the trace ref through the port
    let link_result = adapter.link_trace(entity_id.to_string(), trace_ref.clone()).await;
    assert!(link_result.is_ok());

    // Build contract with linked criterion
    let mut contract = AcceptanceContract::new(entity_id)
        .add_criterion(
            AcceptanceCriterion::new("AC-1", "Feature requirement satisfied")
                .with_trace(trace_ref.clone()),
        );

    // Verify the criterion
    let verify_result = contract.verify_criterion("AC-1");
    assert!(verify_result.is_ok());

    // Check state
    let criterion = contract.get_criterion("AC-1").unwrap();
    assert!(criterion.verified);
    assert!(criterion.trace_ref.is_some());
    assert_eq!(
        criterion.trace_ref.as_ref().unwrap().trace_id,
        trace_ref.trace_id
    );

    // Retrieve traces back from the port (will be empty with noop, but validates the API flow)
    let retrieved_traces = adapter.get_traces(entity_id.to_string()).await.expect("get traces");
    assert_eq!(retrieved_traces.len(), 0); // NoopTraceAdapter returns empty
}

#[test]
fn test_criterion_verification_state_tracking() {
    let entity_id = Uuid::new_v4();

    let mut contract = AcceptanceContract::new(entity_id)
        .add_criterion(AcceptanceCriterion::new("AC-1", "First criterion"))
        .add_criterion(AcceptanceCriterion::new("AC-2", "Second criterion"))
        .add_criterion(AcceptanceCriterion::new("AC-3", "Third criterion"));

    // Initially none verified
    assert_eq!(contract.verified_count(), 0);
    assert!(!contract.all_verified());

    // Verify first criterion
    contract.verify_criterion("AC-1").unwrap();
    assert_eq!(contract.verified_count(), 1);
    assert!(!contract.all_verified());

    // Verify second criterion
    contract.verify_criterion("AC-2").unwrap();
    assert_eq!(contract.verified_count(), 2);
    assert!(!contract.all_verified());

    // Verify third criterion
    contract.verify_criterion("AC-3").unwrap();
    assert_eq!(contract.verified_count(), 3);
    assert!(contract.all_verified());
}
