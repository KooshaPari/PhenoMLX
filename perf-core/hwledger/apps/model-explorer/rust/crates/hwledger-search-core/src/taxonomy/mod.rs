//! Model taxonomy used across ingestion, indexing, and faceted search.
//!
//! All enums derive `Serialize + Deserialize` so they can be embedded inside
//! tantivy documents, lancedb payloads, JSON facet dumps, and serialized
//! search responses.

pub mod arch;
pub mod faceted;
pub mod modality;
pub mod model_kind;

pub use arch::{ArchKind, AttentionKind, MlpKind, RopeVariant};
pub use faceted::Facets;
pub use modality::Modality;
pub use model_kind::ModelKind;
