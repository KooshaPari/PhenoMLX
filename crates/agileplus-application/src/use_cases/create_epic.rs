// SPDX-License-Identifier: MIT OR Apache-2.0
//! Use case: Create a new Epic aggregate.

use std::sync::Arc;

use agileplus_domain::domain::epic::Epic;
use agileplus_domain::ports::epic::EpicRepository;
use agileplus_domain::ports::events::{DomainEvent, DomainEventPublisher};

use crate::dto::{CreateEpicCmd, EpicCreatedOutput};
use crate::error::AppError;

/// Creates an Epic, persists it, then publishes `EpicCreated`.
pub struct CreateEpic {
    repo: Arc<dyn EpicRepository>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl CreateEpic {
    pub fn new(repo: Arc<dyn EpicRepository>, publisher: Arc<dyn DomainEventPublisher>) -> Self {
        Self { repo, publisher }
    }

    pub async fn execute(&self, cmd: CreateEpicCmd) -> Result<EpicCreatedOutput, AppError> {
        let epic = Epic::new(cmd.project_id, &cmd.title)?;

        let id = self.repo.create(&epic).await?;

        self.publisher
            .publish(DomainEvent::EpicCreated {
                id,
                project_id: epic.project_id,
                title: epic.title.clone(),
            })?;

        Ok(EpicCreatedOutput { id })
    }
}
