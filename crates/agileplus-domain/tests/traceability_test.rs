//! Integration tests for the traceability port + noop adapter.

use agileplus_domain::adapters::noop_trace_adapter::NoopTraceAdapter;
use agileplus_domain::error::DomainError;
use agileplus_domain::ports::traceability_port::TraceabilityPort;
use agileplus_domain::traceability::TraceRef;
use chrono::Utc;
use uuid::Uuid;

fn make_trace_ref(trace_id: &str, artifact_type: &str) -> TraceRef {
    TraceRef {
        trace_id: trace_id.to_string(),
        artifact_type: artifact_type.to_string(),
        linked_at: Utc::now(),
    }
}

#[test]
fn trace_ref_serialize_roundtrip() {
    let trace_ref = make_trace_ref("FR-001", "requirement");
    let json = serde_json::to_string(&trace_ref).expect("serialize");
    let deserialized: TraceRef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(trace_ref, deserialized);
}

#[test]
fn trace_ref_partial_eq_by_value() {
    let ts = Utc::now();
    let a = TraceRef {
        trace_id: "FR-002".into(),
        artifact_type: "evidence".into(),
        linked_at: ts,
    };
    let b = TraceRef {
        trace_id: "FR-002".into(),
        artifact_type: "evidence".into(),
        linked_at: ts,
    };
    assert_eq!(a, b);
}

#[test]
fn trace_ref_clone_preserves_fields() {
    let original = make_trace_ref("FR-003", "specification");
    let cloned = original.clone();
    assert_eq!(original, cloned);
    assert_eq!(original.trace_id, cloned.trace_id);
    assert_eq!(original.artifact_type, cloned.artifact_type);
    assert_eq!(original.linked_at, cloned.linked_at);
}

#[tokio::test]
async fn noop_link_trace_returns_ok() {
    let adapter = NoopTraceAdapter;
    let entity_id = Uuid::new_v4();
    let trace_ref = make_trace_ref("FR-100", "requirement");

    let result: Result<(), DomainError> = adapter.link_trace(entity_id.to_string(), trace_ref).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn noop_get_traces_returns_empty_vec() {
    let adapter = NoopTraceAdapter;
    let entity_id = Uuid::new_v4();

    let result: Result<Vec<TraceRef>, DomainError> = adapter.get_traces(entity_id.to_string()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Vec::<TraceRef>::new());
}

#[tokio::test]
async fn noop_link_trace_is_idempotent() {
    let adapter = NoopTraceAdapter;
    let entity_id = Uuid::new_v4();

    for i in 0..3 {
        let trace_ref = make_trace_ref(&format!("FR-2{i}0"), "requirement");
        let result = adapter.link_trace(entity_id.to_string(), trace_ref).await;
        assert!(result.is_ok(), "link_trace {i} should succeed");
    }

    // Noop never records; subsequent get_traces still returns empty.
    let traces = adapter
        .get_traces(entity_id.to_string())
        .await
        .expect("get_traces ok");
    assert!(traces.is_empty());
}
