// SPDX-License-Identifier: MIT OR Apache-2.0
//! `agileplus-application` — hexagonal application / use-case layer.
//!
//! Each use case is a struct holding `Arc<dyn Port + Send + Sync>` deps wired
//! explicitly at call-site. No DI frameworks; no axum/sqlx types.
//!
//! ## Sub-modules
//!
//! - [`dto`]       — Data Transfer Objects
//! - [`use_cases`] — Use case implementations
//!
//! # Structure
//! - `use_cases/` — one module per use case (incl. `triage` for the CLI
//!   triage subcommands backed by `agileplus-triage` + `agileplus-graph`)
//! - `dto/`       — command/output data transfer objects
//! - `error.rs`   — `AppError` (thiserror; never leaks storage details)
//! - `events.rs`  — re-exports `DomainEvent` / `DomainEventPublisher`

pub mod dto;
pub mod error;
pub mod events;
pub mod use_cases;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::RwLock;

    use agileplus_domain::domain::epic::{Epic, EpicStatus};
    use agileplus_domain::domain::feature::Feature;
    use agileplus_domain::domain::state_machine::FeatureState;
    use agileplus_domain::domain::story::{Story, StoryStatus};
    use agileplus_domain::error::DomainError;
    use agileplus_domain::ports::epic::EpicRepository;
    use agileplus_domain::ports::events::{DomainEvent, DomainEventPublisher};
    use agileplus_domain::ports::story::StoryRepository;
    use agileplus_domain::ports::StoragePort;

    use crate::dto::*;
    use crate::error::AppError;
    use crate::use_cases::{
        advance_feature::AdvanceFeature, create_epic::CreateEpic, create_feature::CreateFeature,
        create_story::CreateStory, transition_story::TransitionStory,
    };

    // ── In-memory doubles ────────────────────────────────────────────────────

    /// In-memory Feature store.
    #[derive(Default)]
    struct InMemoryFeatureRepo {
        store: RwLock<HashMap<i64, Feature>>,
        next_id: RwLock<i64>,
    }

    #[async_trait]
    impl StoragePort for InMemoryFeatureRepo {
        async fn create_feature(&self, feature: &Feature) -> Result<i64, DomainError> {
            let mut next = self.next_id.write().await;
            *next += 1;
            let id = *next;
            let mut f = feature.clone();
            f.id = id;
            self.store.write().await.insert(id, f);
            Ok(id)
        }

        async fn get_feature_by_id(&self, id: i64) -> Result<Option<Feature>, DomainError> {
            Ok(self.store.read().await.get(&id).cloned())
        }

        async fn update_feature_state(
            &self,
            id: i64,
            state: FeatureState,
        ) -> Result<(), DomainError> {
            let mut store = self.store.write().await;
            if let Some(f) = store.get_mut(&id) {
                f.state = state;
                Ok(())
            } else {
                Err(DomainError::FeatureNotFound(id.to_string()))
            }
        }

        async fn update_feature(&self, feature: &Feature) -> Result<(), DomainError> {
            let mut store = self.store.write().await;
            if store.contains_key(&feature.id) {
                store.insert(feature.id, feature.clone());
                Ok(())
            } else {
                Err(DomainError::FeatureNotFound(feature.id.to_string()))
            }
        }

        // Minimal stubs for the rest of the trait
        async fn get_feature_by_slug(&self, slug: &str) -> Result<Option<Feature>, DomainError> {
            Ok(self
                .store
                .read()
                .await
                .values()
                .find(|f| f.slug == slug)
                .cloned())
        }
        async fn list_features_by_state(
            &self,
            state: FeatureState,
        ) -> Result<Vec<Feature>, DomainError> {
            Ok(self
                .store
                .read()
                .await
                .values()
                .filter(|f| f.state == state)
                .cloned()
                .collect())
        }
        async fn list_all_features(&self) -> Result<Vec<Feature>, DomainError> {
            Ok(self.store.read().await.values().cloned().collect())
        }
        async fn list_features_by_label(
            &self,
            label: &str,
        ) -> Result<Vec<Feature>, DomainError> {
            Ok(self
                .store
                .read()
                .await
                .values()
                .filter(|f| f.labels.iter().any(|l| l == label))
                .cloned()
                .collect())
        }
        async fn create_work_package(
            &self,
            _: &agileplus_domain::domain::work_package::WorkPackage,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_work_package(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::work_package::WorkPackage>, DomainError>
        {
            Ok(None)
        }
        async fn update_wp_state(
            &self,
            _: i64,
            _: agileplus_domain::domain::work_package::WpState,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn list_wps_by_feature(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::work_package::WorkPackage>, DomainError> {
            Ok(vec![])
        }
        async fn add_wp_dependency(
            &self,
            _: &agileplus_domain::domain::work_package::WpDependency,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn get_wp_dependencies(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::work_package::WpDependency>, DomainError>
        {
            Ok(vec![])
        }
        async fn get_ready_wps(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::work_package::WorkPackage>, DomainError> {
            Ok(vec![])
        }
        async fn append_audit_entry(
            &self,
            _: &agileplus_domain::domain::audit::AuditEntry,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_audit_trail(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::audit::AuditEntry>, DomainError> {
            Ok(vec![])
        }
        async fn get_latest_audit_entry(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::audit::AuditEntry>, DomainError> {
            Ok(None)
        }
        async fn create_evidence(
            &self,
            _: &agileplus_domain::domain::governance::Evidence,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_evidence_by_wp(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::governance::Evidence>, DomainError> {
            Ok(vec![])
        }
        async fn get_evidence_by_fr(
            &self,
            _: &str,
        ) -> Result<Vec<agileplus_domain::domain::governance::Evidence>, DomainError> {
            Ok(vec![])
        }
        async fn create_policy_rule(
            &self,
            _: &agileplus_domain::domain::governance::PolicyRule,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn list_active_policies(
            &self,
        ) -> Result<Vec<agileplus_domain::domain::governance::PolicyRule>, DomainError> {
            Ok(vec![])
        }
        async fn record_metric(
            &self,
            _: &agileplus_domain::domain::metric::Metric,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_metrics_by_feature(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::metric::Metric>, DomainError> {
            Ok(vec![])
        }
        async fn create_governance_contract(
            &self,
            _: &agileplus_domain::domain::governance::GovernanceContract,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_governance_contract(
            &self,
            _: i64,
            _: i32,
        ) -> Result<Option<agileplus_domain::domain::governance::GovernanceContract>, DomainError>
        {
            Ok(None)
        }
        async fn get_latest_governance_contract(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::governance::GovernanceContract>, DomainError>
        {
            Ok(None)
        }
        async fn create_module(
            &self,
            _: &agileplus_domain::domain::module::Module,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_module(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::module::Module>, DomainError> {
            Ok(None)
        }
        async fn get_module_by_slug(
            &self,
            _: &str,
        ) -> Result<Option<agileplus_domain::domain::module::Module>, DomainError> {
            Ok(None)
        }
        async fn update_module(&self, _: i64, _: &str, _: Option<&str>) -> Result<(), DomainError> {
            Ok(())
        }
        async fn delete_module(&self, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn list_root_modules(
            &self,
        ) -> Result<Vec<agileplus_domain::domain::module::Module>, DomainError> {
            Ok(vec![])
        }
        async fn list_child_modules(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::module::Module>, DomainError> {
            Ok(vec![])
        }
        async fn get_module_with_features(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::module::ModuleWithFeatures>, DomainError>
        {
            Ok(None)
        }
        async fn tag_feature_to_module(
            &self,
            _: &agileplus_domain::domain::module::ModuleFeatureTag,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn untag_feature_from_module(&self, _: i64, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn create_cycle(
            &self,
            _: &agileplus_domain::domain::cycle::Cycle,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_cycle(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::cycle::Cycle>, DomainError> {
            Ok(None)
        }
        async fn update_cycle_state(
            &self,
            _: i64,
            _: agileplus_domain::domain::cycle::CycleState,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn list_cycles_by_state(
            &self,
            _: agileplus_domain::domain::cycle::CycleState,
        ) -> Result<Vec<agileplus_domain::domain::cycle::Cycle>, DomainError> {
            Ok(vec![])
        }
        async fn list_cycles_by_module(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::cycle::Cycle>, DomainError> {
            Ok(vec![])
        }
        async fn list_all_cycles(
            &self,
        ) -> Result<Vec<agileplus_domain::domain::cycle::Cycle>, DomainError> {
            Ok(vec![])
        }
        async fn get_cycle_with_features(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::cycle::CycleWithFeatures>, DomainError>
        {
            Ok(None)
        }
        async fn add_feature_to_cycle(
            &self,
            _: &agileplus_domain::domain::cycle::CycleFeature,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn remove_feature_from_cycle(&self, _: i64, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn get_sync_mapping(
            &self,
            _: &str,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::sync_mapping::SyncMapping>, DomainError>
        {
            Ok(None)
        }
        async fn upsert_sync_mapping(
            &self,
            _: &agileplus_domain::domain::sync_mapping::SyncMapping,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn get_sync_mapping_by_plane_id(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<agileplus_domain::domain::sync_mapping::SyncMapping>, DomainError>
        {
            Ok(None)
        }
        async fn delete_sync_mapping(&self, _: &str, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn create_project(
            &self,
            _: &agileplus_domain::domain::project::Project,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_project_by_slug(
            &self,
            _: &str,
        ) -> Result<Option<agileplus_domain::domain::project::Project>, DomainError> {
            Ok(None)
        }
        async fn get_project_by_id(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::project::Project>, DomainError> {
            Ok(None)
        }
        async fn list_all_projects(
            &self,
        ) -> Result<Vec<agileplus_domain::domain::project::Project>, DomainError> {
            Ok(vec![])
        }
        async fn delete_project(&self, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn create_user(
            &self,
            _: &agileplus_domain::domain::user::User,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_user(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::user::User>, DomainError> {
            Ok(None)
        }
        async fn get_user_by_email(
            &self,
            _: &str,
        ) -> Result<Option<agileplus_domain::domain::user::User>, DomainError> {
            Ok(None)
        }
        async fn update_user_status(
            &self,
            _: i64,
            _: agileplus_domain::domain::user::UserStatus,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn update_user_role(
            &self,
            _: i64,
            _: agileplus_domain::domain::user::UserRole,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn list_all_users(
            &self,
        ) -> Result<Vec<agileplus_domain::domain::user::User>, DomainError> {
            Ok(vec![])
        }
        async fn delete_user(&self, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn create_epic(
            &self,
            _: &agileplus_domain::domain::epic::Epic,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_epic(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::epic::Epic>, DomainError> {
            Ok(None)
        }
        async fn update_epic_status(
            &self,
            _: i64,
            _: agileplus_domain::domain::epic::EpicStatus,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn list_epics_by_project(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::epic::Epic>, DomainError> {
            Ok(vec![])
        }
        async fn delete_epic(&self, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn create_story(
            &self,
            _: &agileplus_domain::domain::story::Story,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
        async fn get_story(
            &self,
            _: i64,
        ) -> Result<Option<agileplus_domain::domain::story::Story>, DomainError> {
            Ok(None)
        }
        async fn update_story_status(
            &self,
            _: i64,
            _: agileplus_domain::domain::story::StoryStatus,
        ) -> Result<(), DomainError> {
            Ok(())
        }
        async fn list_stories_by_epic(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::story::Story>, DomainError> {
            Ok(vec![])
        }
        async fn list_stories_by_project(
            &self,
            _: i64,
        ) -> Result<Vec<agileplus_domain::domain::story::Story>, DomainError> {
            Ok(vec![])
        }
        async fn delete_story(&self, _: i64) -> Result<(), DomainError> {
            Ok(())
        }
        async fn upsert_story_by_requirement_id(
            &self,
            _: &agileplus_domain::domain::story::Story,
        ) -> Result<i64, DomainError> {
            Ok(0)
        }
    }

    /// In-memory Story store.
    #[derive(Default)]
    struct InMemoryStoryRepo {
        store: RwLock<HashMap<i64, Story>>,
        next_id: RwLock<i64>,
    }

    #[async_trait]
    impl StoryRepository for InMemoryStoryRepo {
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

    /// In-memory Epic store.
    #[derive(Default)]
    struct InMemoryEpicRepo {
        store: RwLock<HashMap<i64, Epic>>,
        next_id: RwLock<i64>,
    }

    #[async_trait]
    impl EpicRepository for InMemoryEpicRepo {
        async fn create(&self, epic: &Epic) -> Result<i64, DomainError> {
            let mut next = self.next_id.write().await;
            *next += 1;
            let id = *next;
            let mut e = epic.clone();
            e.id = id;
            self.store.write().await.insert(id, e);
            Ok(id)
        }

        async fn get_by_id(&self, id: i64) -> Result<Option<Epic>, DomainError> {
            Ok(self.store.read().await.get(&id).cloned())
        }

        async fn update_status(&self, id: i64, status: EpicStatus) -> Result<(), DomainError> {
            let mut store = self.store.write().await;
            if let Some(e) = store.get_mut(&id) {
                e.status = status;
                Ok(())
            } else {
                Err(DomainError::NotFound(id.to_string()))
            }
        }

        async fn list_by_project(&self, project_id: i64) -> Result<Vec<Epic>, DomainError> {
            Ok(self
                .store
                .read()
                .await
                .values()
                .filter(|e| e.project_id == project_id)
                .cloned()
                .collect())
        }
    }

    /// Spy publisher — records emitted events.
    #[derive(Default)]
    struct SpyPublisher {
        events: RwLock<Vec<DomainEvent>>,
    }

    #[async_trait]
    impl DomainEventPublisher for SpyPublisher {
        async fn publish(&self, event: DomainEvent) -> Result<(), DomainError> {
            self.events.write().await.push(event);
            Ok(())
        }
    }

    impl SpyPublisher {
        async fn emitted(&self) -> Vec<DomainEvent> {
            self.events.read().await.clone()
        }
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    // --- CreateFeature ---

    #[tokio::test]
    async fn create_feature_happy_path() {
        let repo = Arc::new(InMemoryFeatureRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateFeature::new(repo.clone(), pub_.clone());

        let out = uc
            .execute(CreateFeatureCmd {
                slug: "auth".to_string(),
                friendly_name: "Authentication".to_string(),
                spec_hash: None,
                target_branch: None,
            })
            .await
            .unwrap();

        assert_eq!(out.id, 1);
        assert_eq!(out.feature.slug, "auth");

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], DomainEvent::FeatureCreated { slug, .. } if slug == "auth"));
    }

    #[tokio::test]
    async fn create_feature_defaults_target_branch_and_spec_hash() {
        let repo = Arc::new(InMemoryFeatureRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateFeature::new(repo.clone(), pub_.clone());

        let out = uc
            .execute(CreateFeatureCmd {
                slug: "spec-default".to_string(),
                friendly_name: "Spec-driven".to_string(),
                spec_hash: None,
                target_branch: None,
            })
            .await
            .unwrap();

        assert_eq!(out.feature.target_branch, "main");
        assert_eq!(out.feature.spec_hash, [0u8; 32]);

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            DomainEvent::FeatureCreated { slug, .. } if slug == "spec-default"
        ));
    }

    #[tokio::test]
    async fn create_feature_preserves_target_branch_and_spec_hash() {
        let repo = Arc::new(InMemoryFeatureRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateFeature::new(repo.clone(), pub_.clone());
        let spec_hash = [11u8; 32];

        let out = uc
            .execute(CreateFeatureCmd {
                slug: "branched-feature".to_string(),
                friendly_name: "Branch-aware feature".to_string(),
                spec_hash: Some(spec_hash),
                target_branch: Some("feature/login".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(out.feature.target_branch, "feature/login");
        assert_eq!(out.feature.spec_hash, spec_hash);

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            DomainEvent::FeatureCreated { slug, .. } if slug == "branched-feature"
        ));
    }

    // --- AdvanceFeature ---

    #[tokio::test]
    async fn advance_feature_valid_transition() {
        let repo = Arc::new(InMemoryFeatureRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let create_uc = CreateFeature::new(repo.clone(), pub_.clone());
        let advance_uc = AdvanceFeature::new(repo.clone(), pub_.clone());

        let out = create_uc
            .execute(CreateFeatureCmd {
                slug: "feat-a".to_string(),
                friendly_name: "Feature A".to_string(),
                spec_hash: None,
                target_branch: None,
            })
            .await
            .unwrap();

        advance_uc
            .execute(AdvanceFeatureCmd {
                feature_id: out.id,
                target_state: "specified".to_string(),
            })
            .await
            .unwrap();

        let feature = repo.get_feature_by_id(out.id).await.unwrap().unwrap();
        assert_eq!(feature.state, FeatureState::Specified);

        let events = pub_.emitted().await;
        // created + advanced
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[1], DomainEvent::FeatureStateAdvanced { from, to, .. }
            if from == "created" && to == "specified")
        );
    }

    #[tokio::test]
    async fn advance_feature_invalid_transition_rejected() {
        let repo = Arc::new(InMemoryFeatureRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let create_uc = CreateFeature::new(repo.clone(), pub_.clone());
        let advance_uc = AdvanceFeature::new(repo.clone(), pub_.clone());

        let out = create_uc
            .execute(CreateFeatureCmd {
                slug: "feat-b".to_string(),
                friendly_name: "Feature B".to_string(),
                spec_hash: None,
                target_branch: None,
            })
            .await
            .unwrap();

        // Created -> Shipped is not allowed
        let err = advance_uc
            .execute(AdvanceFeatureCmd {
                feature_id: out.id,
                target_state: "shipped".to_string(),
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Domain(_)));
    }

    #[tokio::test]
    async fn advance_feature_not_found() {
        let repo = Arc::new(InMemoryFeatureRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = AdvanceFeature::new(repo.clone(), pub_.clone());

        let err = uc
            .execute(AdvanceFeatureCmd {
                feature_id: 999,
                target_state: "specified".to_string(),
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::NotFound(_)));
    }

    // --- CreateStory ---

    #[tokio::test]
    async fn create_story_happy_path() {
        let repo = Arc::new(InMemoryStoryRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateStory::new(repo.clone(), pub_.clone());

        let out = uc
            .execute(CreateStoryCmd {
                epic_id: 1,
                project_id: 10,
                title: "User can log in".to_string(),
                points: Some(3),
            })
            .await
            .unwrap();

        assert_eq!(out.id, 1);
        assert_eq!(out.story.title, "User can log in");

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            DomainEvent::StoryCreated { epic_id: 1, .. }
        ));
    }

    #[tokio::test]
    async fn create_story_rejects_empty_title() {
        let repo = Arc::new(InMemoryStoryRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateStory::new(repo, pub_);

        let err = uc
            .execute(CreateStoryCmd {
                epic_id: 1,
                project_id: 10,
                title: "".to_string(),
                points: None,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            AppError::Domain(agileplus_domain::error::DomainError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn create_story_rejects_zero_points() {
        let repo = Arc::new(InMemoryStoryRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateStory::new(repo, pub_);

        let err = uc
            .execute(CreateStoryCmd {
                epic_id: 1,
                project_id: 10,
                title: "Some story".to_string(),
                points: Some(0),
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Domain(_)));
    }

    // --- TransitionStory ---

    #[tokio::test]
    async fn transition_story_happy_path() {
        let repo = Arc::new(InMemoryStoryRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let create_uc = CreateStory::new(repo.clone(), pub_.clone());
        let trans_uc = TransitionStory::new(repo.clone(), pub_.clone());

        let out = create_uc
            .execute(CreateStoryCmd {
                epic_id: 2,
                project_id: 20,
                title: "Login flow".to_string(),
                points: None,
            })
            .await
            .unwrap();

        trans_uc
            .execute(TransitionStoryCmd {
                story_id: out.id,
                target_status: StoryStatus::InProgress,
            })
            .await
            .unwrap();

        let story = repo.get_by_id(out.id).await.unwrap().unwrap();
        assert_eq!(story.status, StoryStatus::InProgress);

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[1], DomainEvent::StoryStatusChanged { from, to, .. }
            if from == "todo" && to == "in_progress")
        );
    }

    #[tokio::test]
    async fn transition_story_invalid_transition_rejected() {
        let repo = Arc::new(InMemoryStoryRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let create_uc = CreateStory::new(repo.clone(), pub_.clone());
        let trans_uc = TransitionStory::new(repo.clone(), pub_.clone());

        let out = create_uc
            .execute(CreateStoryCmd {
                epic_id: 2,
                project_id: 20,
                title: "Story skip".to_string(),
                points: None,
            })
            .await
            .unwrap();

        // Todo -> Done is not allowed
        let err = trans_uc
            .execute(TransitionStoryCmd {
                story_id: out.id,
                target_status: StoryStatus::Done,
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Domain(_)));
    }

    #[tokio::test]
    async fn transition_story_not_found() {
        let repo = Arc::new(InMemoryStoryRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = TransitionStory::new(repo, pub_);

        let err = uc
            .execute(TransitionStoryCmd {
                story_id: 999,
                target_status: StoryStatus::InProgress,
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::NotFound(_)));
    }

    // --- CreateEpic ---

    #[tokio::test]
    async fn create_epic_happy_path() {
        let repo = Arc::new(InMemoryEpicRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateEpic::new(repo.clone(), pub_.clone());

        let out = uc
            .execute(CreateEpicCmd {
                project_id: 5,
                title: "Auth Epic".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(out.id, 1);

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            DomainEvent::EpicCreated { project_id: 5, .. }
        ));
    }

    #[tokio::test]
    async fn create_epic_rejects_empty_title() {
        let repo = Arc::new(InMemoryEpicRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateEpic::new(repo, pub_);

        let err = uc
            .execute(CreateEpicCmd {
                project_id: 5,
                title: "   ".to_string(),
            })
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Domain(_)));
    }

    #[tokio::test]
    async fn create_epic_trims_title_and_emits_expected_event() {
        let repo = Arc::new(InMemoryEpicRepo::default());
        let pub_ = Arc::new(SpyPublisher::default());
        let uc = CreateEpic::new(repo.clone(), pub_.clone());

        let out = uc
            .execute(CreateEpicCmd {
                project_id: 42,
                title: "  API hardening  ".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(out.id, 1);

        let stored = repo.get_by_id(1).await.unwrap().unwrap();
        assert_eq!(stored.title, "API hardening");

        let events = pub_.emitted().await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            DomainEvent::EpicCreated {
                project_id: 42,
                title,
                ..
            } if title == "API hardening"
        ));
    }
}
