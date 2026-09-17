// SPDX-License-Identifier: MIT OR Apache-2.0
//! Newtype ID wrappers for compile-time type safety.
//!
//! Wrapping primitive IDs in distinct types prevents the most common
//! domain-bug class — accidentally passing a `UserId` where a `FeatureId`
//! is expected (or vice versa). Each wrapper implements:
//! - `Debug`/`Clone`/`PartialEq`/`Eq`/`Hash` for normal usage
//! - `Serialize`/`Deserialize` (transparent — wire format unchanged)
//! - `Display`/`FromStr` so existing string-based APIs still work
//! - `AsRef<str>` for ergonomic borrow handling
//!
//! All wrappers take the inner string at construction; use `::new()`
//! for unchecked construction and `::parse()` if you want validation.
//!
//! Migration is incremental — start using `UserId` for new code and
//! convert existing call sites as you touch them. Plain `i64` / `String`
//! call sites continue to work unchanged.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::DomainError;

macro_rules! newtype_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Construct without validation.
            #[inline]
            pub fn new(inner: impl Into<String>) -> Self {
                Self(inner.into())
            }

            /// Borrow the inner string.
            #[inline]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Take the inner String, consuming self.
            #[inline]
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = DomainError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if s.is_empty() {
                    Err(DomainError::Validation(format!(
                        "{} must not be empty",
                        stringify!($name)
                    )))
                } else {
                    Ok(Self(s.to_string()))
                }
            }
        }

        impl AsRef<str> for $name {
            #[inline]
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            #[inline]
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            #[inline]
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }
    };
}

// --- PM aggregates ---
newtype_id!(ProjectId, "Project identifier (UUID or short slug).");
newtype_id!(ModuleId, "Module / sub-project identifier.");
newtype_id!(FeatureId, "Feature identifier.");
newtype_id!(EpicId, "Epic identifier.");
newtype_id!(StoryId, "Story identifier.");
newtype_id!(CycleId, "Cycle / sprint identifier.");
newtype_id!(WorkPackageId, "Work package identifier.");
newtype_id!(BacklogId, "Backlog identifier.");
newtype_id!(IntentId, "Intent (strategic goal) identifier.");

// --- People & RBAC ---
newtype_id!(UserId, "User identifier.");
newtype_id!(ApiKeyId, "API key identifier (prefix only).");
newtype_id!(AuditId, "Audit-log entry identifier.");

// --- External / federated ---
newtype_id!(PlaneIssueId, "Upstream Plane.so issue identifier.");
newtype_id!(PlaneStateId, "Upstream Plane.so state identifier.");
newtype_id!(GithubLogin, "GitHub login (owner/repo/actor).");
newtype_id!(BranchName, "Git branch name.");
newtype_id!(CommitSha, "Git commit SHA (40-char hex).");

impl CommitSha {
    /// Validate `s` is a 7- to 40-character hex string (full or short SHA).
    pub fn parse_sha(s: &str) -> Result<Self, DomainError> {
        if !(7..=40).contains(&s.len()) {
            return Err(DomainError::Validation(format!(
                "CommitSha must be 7-40 chars, got {}",
                s.len()
            )));
        }
        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DomainError::Validation(
                "CommitSha must be hex characters only".to_string(),
            ));
        }
        Ok(Self(s.to_ascii_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newtype_round_trip() {
        let id = FeatureId::new("F-123");
        assert_eq!(id.to_string(), "F-123");
        let parsed: FeatureId = "F-123".parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn empty_rejected() {
        assert!("".parse::<FeatureId>().is_err());
    }

    #[test]
    fn serde_transparent() {
        let v = serde_json::to_string(&FeatureId::new("F-1")).unwrap();
        assert_eq!(v, "\"F-1\"");
        let back: FeatureId = serde_json::from_str(&v).unwrap();
        assert_eq!(back, FeatureId::new("F-1"));
    }

    #[test]
    fn commit_sha_validation() {
        assert!(CommitSha::parse_sha("0123abc").is_ok());
        assert!(CommitSha::parse_sha("deadbeef1234567890abcdef1234567890abcdef").is_ok());
        assert!(CommitSha::parse_sha("short").is_err()); // too short
        assert!(CommitSha::parse_sha("not a sha!").is_err()); // invalid chars
    }

    #[test]
    fn as_ref_str() {
        let id = ModuleId::new("payments");
        assert_eq!(id.as_ref(), "payments");
        assert_eq!(id.as_str(), "payments");
    }
}
