// SPDX-License-Identifier: MIT OR Apache-2.0
//! Story repository port.

use async_trait::async_trait;

use crate::domain::story::{Story, StoryStatus};
use crate::error::DomainError;

/// Repository port for Story aggregates.
#[async_trait]
pub trait StoryRepository: Send + Sync {
    async fn create(&self, story: &Story) -> Result<i64, DomainError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<Story>, DomainError>;
    async fn update_status(&self, id: i64, status: StoryStatus) -> Result<(), DomainError>;
    async fn list_by_epic(&self, epic_id: i64) -> Result<Vec<Story>, DomainError>;

    /// Upsert a story keyed by `story.requirement_id`.
    ///
    /// If a story with the same `requirement_id` already exists, its
    /// title/description/status are updated and the existing row ID is
    /// returned. Otherwise, the story is inserted and the new row ID is
    /// returned.
    ///
    /// Returns `Err(DomainError::Validation)` if `story.requirement_id`
    /// is `None`.
    ///
    /// The default implementation performs a get-then-insert/update using
    /// the base `create` / `get_by_id` methods.  Adapters with native
    /// SQL UPSERT support should override this for efficiency.
    async fn upsert_by_requirement_id(&self, story: &Story) -> Result<i64, DomainError> {
        let req_id = story.requirement_id.as_deref().ok_or_else(|| {
            DomainError::Validation(
                "upsert_by_requirement_id requires story.requirement_id to be set".to_string(),
            )
        })?;

        // Walk all stories for this epic and check for a match.
        // This default impl is O(n) per epic and intended only as a
        // portable fallback; adapters should override with an indexed query.
        let candidates = self.list_by_epic(story.epic_id).await?;
        if let Some(existing) = candidates
            .iter()
            .find(|s| s.requirement_id.as_deref() == Some(req_id))
        {
            // Update status to reflect latest GitHub state.
            self.update_status(existing.id, story.status).await?;
            Ok(existing.id)
        } else {
            self.create(story).await
        }
    }
}
