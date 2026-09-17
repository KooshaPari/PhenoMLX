//! Strongly-typed requirement identifiers.
//!
//! Source: [`Tracera/crates/tracera-core/src/ids.rs`](https://example.invalid/Tracera/crates/tracera-core/src/ids.rs)
//! (1:1 port).
//!
//! `RequirementId` is the FR-prefixed id (e.g. `FR-77`, `FR-<uuid>`).
//! `NfrId` is the NFR-prefixed id (e.g. `NFR-PERF-01`, `NFR-<uuid>`).
//!
//! AgilePlus' governance vocabulary already uses stringly-typed `fr_id: String`
//! fields. We keep the newtype wrapper for compile-time safety inside this
//! crate, and add a `to_fr_id_string()` accessor (and `from_fr_id_string`)
//! so that boundary code can move losslessly into and out of the AgilePlus
//! string-typed shape. See `docs/adr/ADR-0001-superset-merge.md` §2.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Create a fresh id with a random UUID suffix.
            pub fn new() -> Self {
                Self(format!("{}-{}", $prefix, Uuid::new_v4()))
            }

            /// Construct from any value, prefixing the canonical `$prefix-` if absent.
            pub fn from_string(value: impl Into<String>) -> Self {
                let value = value.into();
                if value.starts_with(concat!($prefix, "-")) {
                    Self(value)
                } else {
                    Self(format!("{}-{}", $prefix, value))
                }
            }

            /// Construct from any value, prefixing the canonical `$prefix-` if absent.
            ///
            /// Alias for [`Self::from_string`] used at AgilePlus boundary code
            /// (where the field is `fr_id: String`).
            pub fn from_fr_id_string(value: impl Into<String>) -> Self {
                Self::from_string(value)
            }

            /// Parse an id, returning `Err` if the input is empty.
            pub fn parse(value: impl Into<String>) -> Result<Self, String> {
                let value = value.into();
                if value.trim().is_empty() {
                    Err("id cannot be empty".to_string())
                } else {
                    Ok(Self::from_string(value))
                }
            }

            /// Borrow the underlying string.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Lossless conversion to the AgilePlus `String`-shaped boundary type.
            pub fn to_fr_id_string(&self) -> String {
                self.0.clone()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(RequirementId, "FR");
id_type!(NfrId, "NFR");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fr_id_new_is_fr_prefixed() {
        let id = RequirementId::new();
        assert!(id.as_str().starts_with("FR-"));
    }

    #[test]
    fn nfr_id_new_is_nfr_prefixed() {
        let id = NfrId::new();
        assert!(id.as_str().starts_with("NFR-"));
    }

    #[test]
    fn fr_id_from_string_idempotent() {
        let a = RequirementId::from_string("FR-77");
        let b = RequirementId::from_string("FR-77");
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "FR-77");
    }

    #[test]
    fn fr_id_from_string_bare_value_is_prefixed() {
        let id = RequirementId::from_string("77");
        assert_eq!(id.as_str(), "FR-77");
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(RequirementId::parse("").is_err());
        assert!(RequirementId::parse("   ").is_err());
    }

    #[test]
    fn to_fr_id_string_round_trips_with_agileplus_shape() {
        let id = RequirementId::from_string("FR-001");
        let boundary: String = id.to_fr_id_string();
        assert_eq!(boundary, "FR-001");
        let back: RequirementId = RequirementId::from_fr_id_string(boundary);
        assert_eq!(back, id);
    }

    #[test]
    fn display_matches_as_str() {
        let id = RequirementId::from_string("FR-9");
        assert_eq!(format!("{id}"), "FR-9");
        assert_eq!(id.to_string(), id.as_str());
    }

    #[test]
    fn serde_json_roundtrip() {
        let id = RequirementId::from_string("FR-9001");
        let json = serde_json::to_string(&id).unwrap();
        // transparent serde → bare string
        assert_eq!(json, "\"FR-9001\"");
        let back: RequirementId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}
