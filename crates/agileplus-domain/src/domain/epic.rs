//! Epic aggregate — a large body of work scoped to a project.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::DomainError;

/// Lifecycle status of an epic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EpicStatus {
    Backlog,
    Active,
    Review,
    Done,
    Cancelled,
}

impl fmt::Display for EpicStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EpicStatus::Backlog => "backlog",
            EpicStatus::Active => "active",
            EpicStatus::Review => "review",
            EpicStatus::Done => "done",
            EpicStatus::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

impl FromStr for EpicStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "backlog" => Ok(EpicStatus::Backlog),
            "active" => Ok(EpicStatus::Active),
            "review" => Ok(EpicStatus::Review),
            "done" => Ok(EpicStatus::Done),
            "cancelled" => Ok(EpicStatus::Cancelled),
            _ => Err(DomainError::Validation(format!("unknown EpicStatus: {s}"))),
        }
    }
}

impl EpicStatus {
    /// Returns `true` if a transition from `self` to `target` is allowed.
    pub fn can_transition_to(self, target: EpicStatus) -> bool {
        use EpicStatus::*;
        matches!(
            (self, target),
            (Backlog, Active)
                | (Active, Review)
                | (Active, Cancelled)
                | (Review, Done)
                | (Review, Active)
        )
    }
}

/// An epic — a large, named unit of work belonging to a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Epic {
    pub id: i64,
    /// Owning project.
    pub project_id: i64,
    /// Human-readable title — must be non-empty.
    pub title: String,
    pub description: Option<String>,
    pub status: EpicStatus,
    /// Optional owner (user id).
    pub owner_id: Option<i64>,
    /// Epic/requirement id this epic maps to.
    #[serde(default)]
    pub requirement_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Epic {
    /// Construct a new `Epic`. `title` must be non-empty.
    pub fn new(project_id: i64, title: &str) -> Result<Self, DomainError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(DomainError::Validation(
                "epic title must not be empty".to_string(),
            ));
        }
        let now = Utc::now();
        Ok(Self {
            id: 0,
            project_id,
            title: title.to_string(),
            description: None,
            status: EpicStatus::Backlog,
            owner_id: None,
            requirement_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Attempt a status transition. Returns `Err(DomainError::InvalidTransition)`
    /// if the transition is not permitted.
    pub fn transition_status(&mut self, target: EpicStatus) -> Result<(), DomainError> {
        if !self.status.can_transition_to(target) {
            return Err(DomainError::InvalidTransition {
                from: self.status.to_string(),
                to: target.to_string(),
                reason: "not an allowed epic status transition".to_string(),
            });
        }
        self.status = target;
        self.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_epic_construction() {
        let e = Epic::new(1, "Authentication Overhaul").unwrap();
        assert_eq!(e.title, "Authentication Overhaul");
        assert_eq!(e.project_id, 1);
        assert_eq!(e.status, EpicStatus::Backlog);
    }

    #[test]
    fn rejects_empty_title() {
        let err = Epic::new(1, "  ").unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn valid_status_transition() {
        let mut e = Epic::new(1, "Big Feature").unwrap();
        e.transition_status(EpicStatus::Active).unwrap();
        assert_eq!(e.status, EpicStatus::Active);
        e.transition_status(EpicStatus::Review).unwrap();
        assert_eq!(e.status, EpicStatus::Review);
    }

    #[test]
    fn invalid_status_transition_rejected() {
        let mut e = Epic::new(1, "Skipped Epic").unwrap();
        // Backlog -> Done is not allowed
        let err = e.transition_status(EpicStatus::Done).unwrap_err();
        assert!(matches!(err, DomainError::InvalidTransition { .. }));
    }

    #[test]
    fn title_trimmed_on_construction() {
        let e = Epic::new(2, "  Trimmed  ").unwrap();
        assert_eq!(e.title, "Trimmed");
    }
}
