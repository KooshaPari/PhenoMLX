//! AgilePlus sync orchestrator — conflict detection, resolution, and NATS integration.
//!
//! Traceability: FR-SYNC-* / WP09

pub mod conflict;
pub mod error;
#[cfg(feature = "nats")]
pub mod nats;
pub mod orchestrator;
pub mod report;
pub mod resolution;
pub mod store;

pub use conflict::SyncConflict;
pub use error::SyncError;
#[cfg(feature = "nats")]
pub use nats::NatsSyncBridge;
pub use orchestrator::SyncOrchestrator;
pub use report::SyncReport;
pub use resolution::{FieldSource, ResolutionStrategy};
pub use store::SyncMappingStore;
