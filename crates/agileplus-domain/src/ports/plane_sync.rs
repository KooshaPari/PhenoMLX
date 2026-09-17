// SPDX-License-Identifier: MIT OR Apache-2.0
//! Plane.so sync port.

use crate::domain::story::{Story, StoryStatus};
use crate::error::DomainError;

/// Minimal Plane project representation used by the sync adapter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlaneProject {
    pub id: String,
    pub name: String,
    pub identifier: String,
}

/// Minimal Plane issue representation used by the sync adapter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlaneIssue {
    pub id: String,
    pub name: String,
    pub state: Option<String>,
    pub priority: Option<i32>,
    pub sequence_id: Option<i64>,
}

impl PlaneIssue {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        state: Option<String>,
        priority: Option<i32>,
        sequence_id: Option<i64>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            state,
            priority,
            sequence_id,
        }
    }
}

/// Hexagonal port for Plane.so synchronization.
pub trait PlaneSyncPort: Send + Sync {
    fn list_projects(&self) -> Result<Vec<PlaneProject>, DomainError>;

    fn sync_story_to_plane(
        &self,
        project_identifier: &str,
        story: &Story,
    ) -> Result<PlaneIssue, DomainError>;

    fn sync_from_plane(
        &self,
        project_id: i64,
        epic_id: i64,
        issue: &PlaneIssue,
    ) -> Result<Story, DomainError>;
}

pub fn story_status_to_plane_state(status: StoryStatus) -> &'static str {
    match status {
        StoryStatus::Todo => "todo",
        StoryStatus::InProgress => "in_progress",
        StoryStatus::Review => "review",
        StoryStatus::Done => "done",
        StoryStatus::Blocked => "blocked",
        StoryStatus::Cancelled => "cancelled",
    }
}

pub fn plane_state_to_story_status(state: &str) -> Result<StoryStatus, DomainError> {
    match state {
        "todo" => Ok(StoryStatus::Todo),
        "in_progress" => Ok(StoryStatus::InProgress),
        "review" => Ok(StoryStatus::Review),
        "done" => Ok(StoryStatus::Done),
        "blocked" => Ok(StoryStatus::Blocked),
        "cancelled" => Ok(StoryStatus::Cancelled),
        other => Err(DomainError::Validation(format!(
            "unknown Plane story state: {other}"
        ))),
    }
}
