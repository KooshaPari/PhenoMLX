//! Traceability port — hexagonal-architecture trait for linking domain entities to
//! external traceability systems (e.g. Tracera).

use async_trait::async_trait;

use crate::error::DomainError;
use crate::traceability::TraceRef;

/// Hexagonal port for external traceability integration.
///
/// Implementors connect AgilePlus domain entities (Epic, Story, WorkPackage) to traced
/// artifacts in an external system such as Tracera.
#[async_trait]
pub trait TraceabilityPort: Send + Sync {
    /// Create or update a link from a domain entity to a traced artifact.
    async fn link_trace(&self, entity_id: String, trace_ref: TraceRef) -> Result<(), DomainError>;

    /// Retrieve all trace links for a given domain entity.
    /// Returns an empty `Vec` when no traces exist (not an error).
    async fn get_traces(&self, entity_id: String) -> Result<Vec<TraceRef>, DomainError>;
}
