// SPDX-License-Identifier: MIT OR Apache-2.0
//! Use case: Create a Story under an Epic.

use std::sync::Arc;

use agileplus_domain::domain::story::Story;
use agileplus_domain::ports::events::{DomainEvent, DomainEventPublisher};
use agileplus_domain::ports::StoryRepository;

use crate::dto::{CreateStoryCmd, StoryCreatedOutput};
use crate::error::AppError;

/// Creates a Story, persists it, then publishes `StoryCreated`.
pub struct CreateStory {
    repo: Arc<dyn StoryRepository>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl CreateStory {
    pub fn new(repo: Arc<dyn StoryRepository>, publisher: Arc<dyn DomainEventPublisher>) -> Self {
        Self { repo, publisher }
    }

    pub async fn execute(&self, cmd: CreateStoryCmd) -> Result<StoryCreatedOutput, AppError> {
        // Domain invariants: non-empty title, points > 0 when given.
        let story = Story::new(cmd.epic_id, cmd.project_id, &cmd.title, cmd.points)?;

        let id = self.repo.create(&story).await?;

        self.publisher
            .publish(DomainEvent::StoryCreated {
                id,
                epic_id: story.epic_id,
                title: story.title.clone(),
            })?;

        let mut persisted = story;
        persisted.id = id;

        Ok(StoryCreatedOutput {
            id,
            story: persisted,
        })
    }
}
