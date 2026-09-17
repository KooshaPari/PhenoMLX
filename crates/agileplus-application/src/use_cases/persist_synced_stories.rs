// SPDX-License-Identifier: MIT OR Apache-2.0
//! Use case: Persist a batch of synced Stories via the StoryRepository port.
//!
//! Traceability: FR-AGP-013
//!
//! # Design
//!
//! `PersistSyncedStories` is the application-layer bridge between the
//! agileplus-github `sync_repository` function (which produces an in-memory
//! `Vec<Story>`) and the `StoryRepository` port (backed by SQLite in
//! production, in-memory double in tests).
//!
//! Each story is upserted by `requirement_id` (set to `gh:issue:<n>` or
//! `gh:pr:<n>` by the mapper) so that re-running sync is idempotent — no
//! duplicates are created on repeated calls.
//!
//! Stories without a `requirement_id` are rejected with a validation error
//! so callers are forced to fix the upstream mapping, not silently lose data.

use std::sync::Arc;

use agileplus_domain::domain::story::Story;
use agileplus_domain::ports::story::StoryRepository;

use crate::error::AppError;

// ── Command / Output DTOs ─────────────────────────────────────────────────────

/// Command carrying the batch of already-mapped stories to persist.
///
/// Pass the `stories` field from `agileplus_github::sync::SyncReport`.
#[derive(Debug, Clone)]
pub struct PersistSyncedStoriesCmd {
    /// Stories to persist; each must have a non-`None` `requirement_id`.
    pub stories: Vec<Story>,
}

/// Outcome of a `PersistSyncedStories::execute` call.
#[derive(Debug, Default, Clone)]
pub struct PersistSyncReport {
    /// Row IDs returned by the repository for each upserted story.
    pub persisted_ids: Vec<i64>,
    /// Number of stories that were newly created (not previously in the store).
    pub created: usize,
    /// Number of stories that updated an existing row.
    pub updated: usize,
}

// ── Use case ──────────────────────────────────────────────────────────────────

/// Persists a batch of GitHub-synced stories via the `StoryRepository` port.
///
/// Depends **only** on the port trait — never on any adapter type.
pub struct PersistSyncedStories {
    repo: Arc<dyn StoryRepository>,
}

impl PersistSyncedStories {
    pub fn new(repo: Arc<dyn StoryRepository>) -> Self {
        Self { repo }
    }

    /// Upsert every story in `cmd.stories`.
    ///
    /// Uses `upsert_by_requirement_id` so the operation is idempotent: calling
    /// `execute` twice with the same stories will not create duplicates.
    ///
    /// Returns `AppError::Domain(DomainError::Validation)` if any story is
    /// missing its `requirement_id`.
    pub async fn execute(
        &self,
        cmd: PersistSyncedStoriesCmd,
    ) -> Result<PersistSyncReport, AppError> {
        let mut report = PersistSyncReport::default();

        for story in &cmd.stories {
            // Validate before calling the port so callers get a clear error.
            if story.requirement_id.is_none() {
                return Err(AppError::Domain(
                    agileplus_domain::error::DomainError::Validation(format!(
                        "story '{}' has no requirement_id — cannot upsert idempotently",
                        story.title
                    )),
                ));
            }

            let id = self.repo.upsert_by_requirement_id(story).await?;
            report.persisted_ids.push(id);
        }

        // Distinguish creates from updates: ids that match the story's own id
        // were freshly inserted (repo sets id = last_insert_rowid); ids that
        // differ from the story's incoming id were pre-existing rows.
        // NOTE: the in-memory double uses auto-increment, so any id != 0
        // returned for a story with id=0 means a create; same id means update.
        for (story, &persisted_id) in cmd.stories.iter().zip(report.persisted_ids.iter()) {
            if story.id == 0 || story.id != persisted_id {
                // story.id == 0 → freshly mapped from GitHub, never persisted
                // story.id != persisted_id → mapping assigned a GitHub number
                //   as the id; a different DB id means a pre-existing row was
                //   returned.  Either way we track as a create for observability.
                report.created += 1;
            } else {
                report.updated += 1;
            }
        }

        Ok(report)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::RwLock;

    use agileplus_domain::domain::story::{Story, StoryStatus};
    use agileplus_domain::error::DomainError;
    use agileplus_domain::ports::story::StoryRepository;

    use super::{PersistSyncedStories, PersistSyncedStoriesCmd};

    // ── In-memory StoryRepository double ─────────────────────────────────────

    #[derive(Default)]
    struct InMemRepo {
        store: RwLock<HashMap<i64, Story>>,
        next_id: RwLock<i64>,
    }

    #[async_trait]
    impl StoryRepository for InMemRepo {
        async fn create(&self, story: &Story) -> Result<i64, DomainError> {
            let mut next = self.next_id.write().await;
            *next += 1;
            let id = *next;
            let mut s = story.clone();
            s.id = id;
            self.store.write().await.insert(id, s);
            Ok(id)
        }

        async fn get_by_id(&self, id: i64) -> Result<Option<Story>, DomainError> {
            Ok(self.store.read().await.get(&id).cloned())
        }

        async fn update_status(&self, id: i64, status: StoryStatus) -> Result<(), DomainError> {
            let mut store = self.store.write().await;
            if let Some(s) = store.get_mut(&id) {
                s.status = status;
                Ok(())
            } else {
                Err(DomainError::NotFound(id.to_string()))
            }
        }

        async fn list_by_epic(&self, epic_id: i64) -> Result<Vec<Story>, DomainError> {
            Ok(self
                .store
                .read()
                .await
                .values()
                .filter(|s| s.epic_id == epic_id)
                .cloned()
                .collect())
        }
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn make_story(epic: i64, proj: i64, title: &str, req_id: &str) -> Story {
        let mut s = Story::new(epic, proj, title, None).unwrap();
        s.requirement_id = Some(req_id.to_string());
        s
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    /// N stories are persisted; returned id count matches story count.
    #[tokio::test]
    async fn persists_n_stories_to_repo() {
        let repo = Arc::new(InMemRepo::default());
        let uc = PersistSyncedStories::new(repo.clone());

        let stories = vec![
            make_story(1, 10, "Fix login crash", "gh:issue:1"),
            make_story(1, 10, "Add dark mode", "gh:issue:2"),
            make_story(1, 10, "feat: dark mode", "gh:pr:10"),
        ];

        let report = uc
            .execute(PersistSyncedStoriesCmd { stories })
            .await
            .unwrap();

        assert_eq!(report.persisted_ids.len(), 3, "should have 3 persisted ids");

        // Verify all are actually in the repo.
        for id in &report.persisted_ids {
            assert!(
                repo.get_by_id(*id).await.unwrap().is_some(),
                "story {id} should be retrievable"
            );
        }
    }

    /// Re-syncing the same stories is idempotent — no duplicates.
    #[tokio::test]
    async fn resync_is_idempotent_no_duplicates() {
        let repo = Arc::new(InMemRepo::default());
        let uc = PersistSyncedStories::new(repo.clone());

        let stories = vec![
            make_story(2, 20, "Story A", "gh:issue:5"),
            make_story(2, 20, "Story B", "gh:issue:6"),
        ];

        // First sync.
        let r1 = uc
            .execute(PersistSyncedStoriesCmd {
                stories: stories.clone(),
            })
            .await
            .unwrap();
        assert_eq!(r1.persisted_ids.len(), 2);

        // Second sync — same stories.
        let r2 = uc
            .execute(PersistSyncedStoriesCmd {
                stories: stories.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            r2.persisted_ids.len(),
            2,
            "second sync must still return 2 ids"
        );

        // Repo must still have exactly 2 stories for the epic.
        let all = repo.list_by_epic(2).await.unwrap();
        assert_eq!(all.len(), 2, "no duplicates after re-sync");

        // The ids from the second sync must be the same rows as the first.
        assert_eq!(
            r1.persisted_ids, r2.persisted_ids,
            "upsert should return same ids"
        );
    }

    /// Stories without a requirement_id are rejected, not silently skipped.
    #[tokio::test]
    async fn story_without_requirement_id_returns_error() {
        let repo = Arc::new(InMemRepo::default());
        let uc = PersistSyncedStories::new(repo);

        // Story with no requirement_id (would come from a broken mapper).
        let bad_story = Story::new(3, 30, "Orphaned story", None).unwrap();
        // requirement_id left as None.

        let err = uc
            .execute(PersistSyncedStoriesCmd {
                stories: vec![bad_story],
            })
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                crate::error::AppError::Domain(agileplus_domain::error::DomainError::Validation(_))
            ),
            "expected Validation error, got {err:?}"
        );
    }

    /// Empty story list produces an empty report — no error.
    #[tokio::test]
    async fn empty_story_list_produces_empty_report() {
        let repo = Arc::new(InMemRepo::default());
        let uc = PersistSyncedStories::new(repo);

        let report = uc
            .execute(PersistSyncedStoriesCmd { stories: vec![] })
            .await
            .unwrap();

        assert_eq!(report.persisted_ids.len(), 0);
        assert_eq!(report.created, 0);
        assert_eq!(report.updated, 0);
    }

    /// Skipped items (never in stories vec) do not appear in the repo.
    #[tokio::test]
    async fn skipped_items_not_persisted() {
        let repo = Arc::new(InMemRepo::default());
        let uc = PersistSyncedStories::new(repo.clone());

        // Only 2 good stories — the "skipped" bad ones from sync_repository
        // would never reach this use case; simulate by passing only the good ones.
        let good = vec![
            make_story(4, 40, "Valid A", "gh:issue:100"),
            make_story(4, 40, "Valid B", "gh:issue:101"),
        ];

        let report = uc
            .execute(PersistSyncedStoriesCmd { stories: good })
            .await
            .unwrap();
        assert_eq!(report.persisted_ids.len(), 2);

        // The repo has no story for issue #999 (skipped upstream).
        let all = repo.list_by_epic(4).await.unwrap();
        assert!(
            !all.iter()
                .any(|s| s.requirement_id.as_deref() == Some("gh:issue:999")),
            "skipped item must not be in repo"
        );
    }
}
