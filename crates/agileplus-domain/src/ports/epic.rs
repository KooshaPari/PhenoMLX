// SPDX-License-Identifier: MIT OR Apache-2.0
//! Epic repository port.

use async_trait::async_trait;

use crate::domain::epic::{Epic, EpicStatus};
use crate::error::DomainError;

/// Repository port for Epic aggregates.
#[async_trait]
pub trait EpicRepository: Send + Sync {
    async fn create(&self, epic: &Epic) -> Result<i64, DomainError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<Epic>, DomainError>;
    async fn update_status(&self, id: i64, status: EpicStatus) -> Result<(), DomainError>;
    async fn list_by_project(&self, project_id: i64) -> Result<Vec<Epic>, DomainError>;
}
