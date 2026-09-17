// SPDX-License-Identifier: MIT OR Apache-2.0
use std::path::PathBuf;

use async_trait::async_trait;

use agileplus_application::error::AppError;

use crate::commands::{list_projects, version, worklog};

fn storage_error(err: impl std::fmt::Display) -> AppError {
    AppError::Storage(Box::new(std::io::Error::other(err.to_string())))
}

#[derive(Debug, Clone)]
pub struct Context {
    db_path: PathBuf,
}

impl Context {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }
}

#[async_trait]
pub trait SubcommandAsync {
    async fn execute(&self, ctx: &mut Context) -> Result<(), AppError>;
}

#[async_trait]
impl SubcommandAsync for list_projects::ListProjectsArgs {
    async fn execute(&self, ctx: &mut Context) -> Result<(), AppError> {
        let storage = agileplus_sqlite::SqliteStorageAdapter::new(ctx.db_path())
            .map_err(|err| AppError::Storage(Box::new(err)))?;

        list_projects::run(self, &storage)
            .await
            .map_err(storage_error)
    }
}

#[async_trait]
impl SubcommandAsync for worklog::WorklogArgs {
    async fn execute(&self, _ctx: &mut Context) -> Result<(), AppError> {
        worklog::run(self).map_err(storage_error)
    }
}

#[async_trait]
impl SubcommandAsync for version::VersionArgs {
    async fn execute(&self, _ctx: &mut Context) -> Result<(), AppError> {
        version::run();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::commands::version::VersionArgs;
    use crate::commands::worklog::WorklogAction;

    fn accepts_dyn_command(_command: &dyn SubcommandAsync) {}

    fn schema_command() -> worklog::WorklogArgs {
        worklog::WorklogArgs {
            dir: PathBuf::from("."),
            action: WorklogAction::Schema,
        }
    }

    #[test]
    fn subcommand_async_trait_is_dyn_compatible() {
        let command = schema_command();
        accepts_dyn_command(&command);
    }

    #[tokio::test]
    async fn worklog_schema_executes_via_dyn_trait() {
        let command = schema_command();
        let dyn_command: &dyn SubcommandAsync = &command;
        let mut ctx = Context::new("agileplus.db");

        let result = dyn_command.execute(&mut ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn version_executes_via_dyn_trait() {
        let command = VersionArgs;
        let dyn_command: &dyn SubcommandAsync = &command;
        let mut ctx = Context::new("agileplus.db");

        let result = dyn_command.execute(&mut ctx).await;
        assert!(result.is_ok());
    }
}
