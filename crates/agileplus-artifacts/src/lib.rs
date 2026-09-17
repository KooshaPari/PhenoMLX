//! AgilePlus artifact storage layer.
//!
//! Provides artifact storage with S3/MinIO backend and an in-memory
//! implementation for tests and local workflows.

pub mod store;

pub use store::{ArtifactError, ArtifactStore, InMemoryArtifactStore, S3ArtifactStore};
