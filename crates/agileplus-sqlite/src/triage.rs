// SPDX-License-Identifier: MIT OR Apache-2.0
//! SQLite-backed triage adapter built on top of the backlog repository methods.

use std::path::Path;

use agileplus_domain::{
    domain::backlog::BacklogStatus,
    ports::{ContentStoragePort, TriageError, TriageOutcome, TriagePort, TriageTicket},
};
use async_trait::async_trait;

use crate::SqliteStorageAdapter;

/// Thin triage adapter that reuses the existing backlog persistence surface.
pub struct SqliteTriageAdapter {
    storage: SqliteStorageAdapter,
}

impl SqliteTriageAdapter {
    pub fn new(db_path: &Path) -> Result<Self, TriageError> {
        let storage = SqliteStorageAdapter::new(db_path).map_err(TriageError::from)?;
        Self::from_storage(storage)
    }

    pub fn in_memory() -> Result<Self, TriageError> {
        let storage = SqliteStorageAdapter::in_memory().map_err(TriageError::from)?;
        Self::from_storage(storage)
    }

    pub fn storage(&self) -> &SqliteStorageAdapter {
        &self.storage
    }

    fn from_storage(storage: SqliteStorageAdapter) -> Result<Self, TriageError> {
        ensure_backlog_storage(&storage)?;
        Ok(Self { storage })
    }
}

fn ensure_backlog_storage(storage: &SqliteStorageAdapter) -> Result<(), TriageError> {
    let sql = include_str!("migrations/016_create_backlog_items.sql")
        .split("-- DOWN")
        .next()
        .unwrap_or_default();
    storage
        .conn_for_bench()
        .map_err(TriageError::from)?
        .execute_batch(sql)
        .map_err(|err| TriageError::Storage(err.to_string()))
}

#[async_trait]
impl TriagePort for SqliteTriageAdapter {
    async fn next_ticket(&self) -> Result<TriageTicket, TriageError> {
        let item = self
            .storage
            .pop_next_backlog_item()
            .await
            .map_err(TriageError::from)?
            .ok_or(TriageError::NoTicketAvailable)?;

        Ok(item.into())
    }

    async fn record_outcome(&self, id: &str, outcome: TriageOutcome) -> Result<(), TriageError> {
        let parsed_id = id
            .parse::<i64>()
            .map_err(|_| TriageError::InvalidTicketId(id.to_string()))?;

        let existing = self
            .storage
            .get_backlog_item(parsed_id)
            .await
            .map_err(TriageError::from)?;
        if existing.is_none() {
            return Err(TriageError::TicketNotFound(id.to_string()));
        }

        let status = match outcome {
            TriageOutcome::Accepted => BacklogStatus::Triaged,
            TriageOutcome::Dismissed => BacklogStatus::Dismissed,
        };

        self.storage
            .update_backlog_status(parsed_id, status)
            .await
            .map_err(TriageError::from)
    }
}
