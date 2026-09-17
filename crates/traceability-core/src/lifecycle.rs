//! Feature lifecycle state machine.
//!
//! Source: [`AgilePlus/crates/agileplus-domain/src/domain/state_machine.rs`](https://example.invalid/AgilePlus/crates/agileplus-domain/src/domain/state_machine.rs)
//! (1:1 port of the 8-stage linear lifecycle). `DomainError` is replaced with
//! [`LifecycleError`] so this crate stays self-contained.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// States in the feature lifecycle (8 stages).
///
/// Source: AgilePlus `state_machine.rs:13-22`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureState {
    Created,
    Specified,
    Researched,
    Planned,
    Implementing,
    Validated,
    Shipped,
    Retrospected,
}

impl fmt::Display for FeatureState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Created => "created",
            Self::Specified => "specified",
            Self::Researched => "researched",
            Self::Planned => "planned",
            Self::Implementing => "implementing",
            Self::Validated => "validated",
            Self::Shipped => "shipped",
            Self::Retrospected => "retrospected",
        };
        write!(f, "{s}")
    }
}

impl FromStr for FeatureState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "created" => Ok(Self::Created),
            "specified" => Ok(Self::Specified),
            "researched" => Ok(Self::Researched),
            "planned" => Ok(Self::Planned),
            "implementing" => Ok(Self::Implementing),
            "validated" => Ok(Self::Validated),
            "shipped" => Ok(Self::Shipped),
            "retrospected" => Ok(Self::Retrospected),
            _ => Err(format!("unknown FeatureState: {s}")),
        }
    }
}

/// A recorded state transition.
#[derive(Debug, Clone)]
pub struct Transition {
    pub from: FeatureState,
    pub to: FeatureState,
}

/// The result of a successful state machine transition.
#[derive(Debug, Clone)]
pub struct TransitionResult {
    pub transition: Transition,
    pub timestamp: DateTime<Utc>,
}

/// Lifecycle transition errors (replaces AgilePlus `DomainError::InvalidTransition`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid transition {from} -> {to}: {reason}")]
pub struct LifecycleError {
    /// Source state.
    pub from: String,
    /// Target state.
    pub to: String,
    /// Human-readable reason.
    pub reason: String,
}

impl FeatureState {
    /// Attempt a transition to `target`. Returns [`LifecycleError`] when the
    /// transition is not allowed by the linear lifecycle.
    pub fn transition(self, target: FeatureState) -> Result<TransitionResult, LifecycleError> {
        let allowed = matches!(
            (self, target),
            (FeatureState::Created, FeatureState::Specified)
                | (FeatureState::Specified, FeatureState::Researched)
                | (FeatureState::Researched, FeatureState::Planned)
                | (FeatureState::Planned, FeatureState::Implementing)
                | (FeatureState::Implementing, FeatureState::Validated)
                | (FeatureState::Validated, FeatureState::Shipped)
                | (FeatureState::Shipped, FeatureState::Retrospected)
        );
        if !allowed {
            return Err(LifecycleError {
                from: self.to_string(),
                to: target.to_string(),
                reason: "not an allowed lifecycle step".to_string(),
            });
        }
        Ok(TransitionResult {
            transition: Transition {
                from: self,
                to: target,
            },
            timestamp: Utc::now(),
        })
    }

    /// Ordered list of all lifecycle stages.
    pub fn all() -> &'static [FeatureState] {
        &[
            FeatureState::Created,
            FeatureState::Specified,
            FeatureState::Researched,
            FeatureState::Planned,
            FeatureState::Implementing,
            FeatureState::Validated,
            FeatureState::Shipped,
            FeatureState::Retrospected,
        ]
    }

    /// Monotonic ordinal for lifecycle comparisons.
    pub fn ordinal(self) -> u8 {
        match self {
            FeatureState::Created => 0,
            FeatureState::Specified => 1,
            FeatureState::Researched => 2,
            FeatureState::Planned => 3,
            FeatureState::Implementing => 4,
            FeatureState::Validated => 5,
            FeatureState::Shipped => 6,
            FeatureState::Retrospected => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_lifecycle_transition_succeeds() {
        let result = FeatureState::Created
            .transition(FeatureState::Specified)
            .unwrap();
        assert_eq!(result.transition.from, FeatureState::Created);
        assert_eq!(result.transition.to, FeatureState::Specified);
    }

    #[test]
    fn invalid_transition_returns_error() {
        let err = FeatureState::Created
            .transition(FeatureState::Shipped)
            .unwrap_err();
        assert_eq!(err.from, "created");
        assert_eq!(err.to, "shipped");
    }

    #[test]
    fn backward_transition_rejected() {
        let err = FeatureState::Specified
            .transition(FeatureState::Created)
            .unwrap_err();
        assert_eq!(err.from, "specified");
        assert_eq!(err.to, "created");
    }

    #[test]
    fn full_happy_path_lifecycle() {
        let states = FeatureState::all();
        for window in states.windows(2) {
            let result = window[0].transition(window[1]);
            assert!(
                result.is_ok(),
                "transition {:?} -> {:?} should succeed",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn feature_state_from_str_roundtrips() {
        for s in [
            "created",
            "specified",
            "researched",
            "planned",
            "implementing",
            "validated",
            "shipped",
            "retrospected",
        ] {
            let state: FeatureState = s.parse().unwrap();
            assert_eq!(state.to_string(), s);
        }
    }

    #[test]
    fn eight_stages_present() {
        assert_eq!(FeatureState::all().len(), 8);
    }
}
