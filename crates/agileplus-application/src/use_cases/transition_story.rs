// SPDX-License-Identifier: MIT OR Apache-2.0
//! Use case: Transition a Story's status (e.g. Todo -> InProgress).

use std::sync::Arc;

use agileplus_domain::ports::events::{DomainEvent, DomainEventPublisher};
use agileplus_domain::ports::StoryRepository;

use crate::dto::TransitionStoryCmd;
use crate::error::AppError;

/// Advances a Story's status through allowed transitions.
pub struct TransitionStory {
    repo: Arc<dyn StoryRepository>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl TransitionStory {
    pub fn new(repo: Arc<dyn StoryRepository>, publisher: Arc<dyn DomainEventPublisher>) -> Self {
        Self { repo, publisher }
    }

    pub async fn execute(&self, cmd: TransitionStoryCmd) -> Result<(), AppError> {
        let mut story = self
            .repo
            .get_by_id(cmd.story_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("story {}", cmd.story_id)))?;

        let from = story.status.to_string();

        // Domain invariant enforced inside `transition_status`.
        story.transition_status(cmd.target_status)?;

        self.repo.update_status(cmd.story_id, story.status).await?;

        self.publisher
            .publish(DomainEvent::StoryStatusChanged {
                id: cmd.story_id,
                from,
                to: story.status.to_string(),
            })?;

        Ok(())
    }
}
