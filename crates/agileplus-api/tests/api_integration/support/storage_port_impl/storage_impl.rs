use agileplus_domain::domain::audit::AuditEntry;
use agileplus_domain::domain::cycle::{Cycle, CycleFeature, CycleState, CycleWithFeatures};
use agileplus_domain::domain::epic::{Epic, EpicStatus};
use agileplus_domain::domain::governance::{Evidence, GovernanceContract, PolicyRule};
use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::metric::Metric;
use agileplus_domain::domain::module::{Module, ModuleFeatureTag, ModuleWithFeatures};
use agileplus_domain::domain::project::Project;
use agileplus_domain::domain::state_machine::FeatureState;
use agileplus_domain::domain::story::{Story, StoryStatus};
use agileplus_domain::domain::sync_mapping::SyncMapping;
use agileplus_domain::domain::user::User;
use agileplus_domain::domain::work_package::{WorkPackage, WpDependency, WpState};
use agileplus_domain::error::DomainError;
use agileplus_domain::ports::StoragePort;
use async_trait::async_trait;

use super::super::storage::MockStorage;
use super::{feature, audit, cycle, evidence, metrics, module, policy, sync_mapping};

#[async_trait]
impl StoragePort for MockStorage {
    async fn create_feature(
        &self,
        f: &Feature,
    ) -> Result<i64, DomainError> {
        feature::create_feature(self, f).await
    }

    async fn get_feature_by_slug(&self, slug: &str) -> Result<Option<Feature>, DomainError> {
        feature::get_feature_by_slug(self, slug).await
    }

    async fn get_feature_by_id(&self, id: i64) -> Result<Option<Feature>, DomainError> {
        feature::get_feature_by_id(self, id).await
    }

    async fn update_feature_state(
        &self,
        id: i64,
        state: FeatureState,
    ) -> Result<(), DomainError> {
        feature::update_feature_state(self, id, state).await
    }

    async fn list_features_by_state(
        &self,
        state: FeatureState,
    ) -> Result<Vec<Feature>, DomainError> {
        feature::list_features_by_state(self, state).await
    }

    async fn list_all_features(&self) -> Result<Vec<Feature>, DomainError> {
        feature::list_all_features(self).await
    }

    async fn create_work_package(
        &self,
        wp: &agileplus_domain::domain::work_package::WorkPackage,
    ) -> Result<i64, DomainError> {
        super::work_package::create_work_package(self, wp).await
    }

    async fn get_work_package(&self, id: i64) -> Result<Option<WorkPackage>, DomainError> {
        super::work_package::get_work_package(self, id).await
    }

    async fn update_wp_state(&self, id: i64, state: WpState) -> Result<(), DomainError> {
        super::work_package::update_wp_state(self, id, state).await
    }

    async fn list_wps_by_feature(&self, feature_id: i64) -> Result<Vec<WorkPackage>, DomainError> {
        super::work_package::list_wps_by_feature(self, feature_id).await
    }

    async fn add_wp_dependency(&self, dep: &WpDependency) -> Result<(), DomainError> {
        super::work_package::add_wp_dependency(self, dep).await
    }

    async fn get_wp_dependencies(&self, wp_id: i64) -> Result<Vec<WpDependency>, DomainError> {
        super::work_package::get_wp_dependencies(self, wp_id).await
    }

    async fn get_ready_wps(&self, feature_id: i64) -> Result<Vec<WorkPackage>, DomainError> {
        super::work_package::get_ready_wps(self, feature_id).await
    }

    async fn append_audit_entry(
        &self,
        entry: &AuditEntry,
    ) -> Result<i64, DomainError> {
        audit::append_audit_entry(self, entry).await
    }

    async fn get_audit_trail(
        &self,
        feature_id: i64,
    ) -> Result<Vec<AuditEntry>, DomainError> {
        audit::get_audit_trail(self, feature_id).await
    }

    async fn get_latest_audit_entry(
        &self,
        feature_id: i64,
    ) -> Result<Option<AuditEntry>, DomainError> {
        audit::get_latest_audit_entry(self, feature_id).await
    }

    async fn create_evidence(
        &self,
        e: &Evidence,
    ) -> Result<i64, DomainError> {
        evidence::create_evidence(self, e).await
    }

    async fn get_evidence_by_wp(
        &self,
        wp_id: i64,
    ) -> Result<Vec<Evidence>, DomainError> {
        evidence::get_evidence_by_wp(self, wp_id).await
    }

    async fn get_evidence_by_fr(
        &self,
        fr_id: &str,
    ) -> Result<Vec<Evidence>, DomainError> {
        evidence::get_evidence_by_fr(self, fr_id).await
    }

    async fn create_policy_rule(
        &self,
        r: &PolicyRule,
    ) -> Result<i64, DomainError> {
        policy::create_policy_rule(self, r).await
    }

    async fn list_active_policies(
        &self,
    ) -> Result<Vec<PolicyRule>, DomainError> {
        policy::list_active_policies(self).await
    }

    async fn record_metric(&self, m: &Metric) -> Result<i64, DomainError> {
        metrics::record_metric(self, m).await
    }

    async fn get_metrics_by_feature(
        &self,
        feature_id: i64,
    ) -> Result<Vec<Metric>, DomainError> {
        metrics::get_metrics_by_feature(self, feature_id).await
    }

    async fn create_governance_contract(
        &self,
        _c: &GovernanceContract,
    ) -> Result<i64, DomainError> {
        Ok(1)
    }

    async fn get_governance_contract(
        &self,
        feature_id: i64,
        version: i32,
    ) -> Result<Option<GovernanceContract>, DomainError> {
        let found = self
            .governance
            .lock()
            .expect("governance lock poisoned")
            .iter()
            .find(|c| c.feature_id == feature_id && c.version == version)
            .cloned();
        Ok(found)
    }

    async fn get_latest_governance_contract(
        &self,
        feature_id: i64,
    ) -> Result<Option<GovernanceContract>, DomainError> {
        let found = self
            .governance
            .lock()
            .expect("governance lock poisoned")
            .iter()
            .filter(|c| c.feature_id == feature_id)
            .max_by_key(|c| c.version)
            .cloned();
        Ok(found)
    }

    async fn create_module(
        &self,
        module: &Module,
    ) -> Result<i64, DomainError> {
        module::create_module(self, module).await
    }

    async fn get_module(
        &self,
        id: i64,
    ) -> Result<Option<Module>, DomainError> {
        module::get_module(self, id).await
    }

    async fn get_module_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<Module>, DomainError> {
        module::get_module_by_slug(self, slug).await
    }

    async fn update_module(
        &self,
        id: i64,
        friendly_name: &str,
        description: Option<&str>,
    ) -> Result<(), DomainError> {
        module::update_module(self, id, friendly_name, description).await
    }

    async fn delete_module(&self, id: i64) -> Result<(), DomainError> {
        module::delete_module(self, id).await
    }

    async fn list_root_modules(&self) -> Result<Vec<Module>, DomainError> {
        module::list_root_modules(self).await
    }

    async fn list_child_modules(
        &self,
        parent_id: i64,
    ) -> Result<Vec<Module>, DomainError> {
        module::list_child_modules(self, parent_id).await
    }

    async fn get_module_with_features(
        &self,
        id: i64,
    ) -> Result<Option<ModuleWithFeatures>, DomainError> {
        module::get_module_with_features(self, id).await
    }

    async fn create_cycle(&self, cycle: &Cycle) -> Result<i64, DomainError> {
        cycle::create_cycle(self, cycle).await
    }

    async fn get_cycle(
        &self,
        id: i64,
    ) -> Result<Option<Cycle>, DomainError> {
        cycle::get_cycle(self, id).await
    }

    async fn update_cycle_state(
        &self,
        id: i64,
        state: CycleState,
    ) -> Result<(), DomainError> {
        cycle::update_cycle_state(self, id, state).await
    }

    async fn list_cycles_by_state(
        &self,
        state: CycleState,
    ) -> Result<Vec<Cycle>, DomainError> {
        cycle::list_cycles_by_state(self, state).await
    }

    async fn list_cycles_by_module(
        &self,
        module_id: i64,
    ) -> Result<Vec<Cycle>, DomainError> {
        cycle::list_cycles_by_module(self, module_id).await
    }

    async fn list_all_cycles(&self) -> Result<Vec<Cycle>, DomainError> {
        cycle::list_all_cycles(self).await
    }

    async fn get_cycle_with_features(
        &self,
        id: i64,
    ) -> Result<Option<CycleWithFeatures>, DomainError> {
        cycle::get_cycle_with_features(self, id).await
    }

    async fn tag_feature_to_module(
        &self,
        tag: &ModuleFeatureTag,
    ) -> Result<(), DomainError> {
        cycle::tag_feature_to_module(self, tag).await
    }

    async fn untag_feature_from_module(
        &self,
        module_id: i64,
        feature_id: i64,
    ) -> Result<(), DomainError> {
        cycle::untag_feature_from_module(self, module_id, feature_id).await
    }

    async fn add_feature_to_cycle(
        &self,
        entry: &CycleFeature,
    ) -> Result<(), DomainError> {
        cycle::add_feature_to_cycle(self, entry).await
    }

    async fn remove_feature_from_cycle(
        &self,
        cycle_id: i64,
        feature_id: i64,
    ) -> Result<(), DomainError> {
        cycle::remove_feature_from_cycle(self, cycle_id, feature_id).await
    }

    async fn get_sync_mapping(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<SyncMapping>, DomainError> {
        sync_mapping::get_sync_mapping(self, entity_type, entity_id).await
    }

    async fn upsert_sync_mapping(
        &self,
        mapping: &SyncMapping,
    ) -> Result<(), DomainError> {
        sync_mapping::upsert_sync_mapping(self, mapping).await
    }

    async fn get_sync_mapping_by_plane_id(
        &self,
        entity_type: &str,
        plane_issue_id: &str,
    ) -> Result<Option<SyncMapping>, DomainError> {
        sync_mapping::get_sync_mapping_by_plane_id(self, entity_type, plane_issue_id).await
    }

    async fn delete_sync_mapping(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<(), DomainError> {
        sync_mapping::delete_sync_mapping(self, entity_type, entity_id).await
    }

    async fn create_project(
        &self,
        project: &Project,
    ) -> Result<i64, DomainError> {
        let mut projects = self.projects.lock().expect("projects lock poisoned");
        let id = (projects.len() as i64) + 1;
        let mut p = project.clone();
        p.id = id;
        projects.push(p);
        Ok(id)
    }

    async fn get_project_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<Project>, DomainError> {
        let found = self
            .projects
            .lock()
            .expect("projects lock poisoned")
            .iter()
            .find(|p| p.slug == slug)
            .cloned();
        Ok(found)
    }

    async fn list_all_projects(&self) -> Result<Vec<Project>, DomainError> {
        let all = self.projects.lock().expect("projects lock poisoned").clone();
        Ok(all)
    }

    async fn create_epic(&self, epic: &Epic) -> Result<i64, DomainError> {
        let mut epics = self.epics.lock().expect("epics lock poisoned");
        let id = (epics.len() as i64) + 1;
        let mut e = epic.clone();
        e.id = id;
        epics.push(e);
        Ok(id)
    }

    async fn get_epic(&self, id: i64) -> Result<Option<Epic>, DomainError> {
        let found = self.epics.lock().expect("epics lock poisoned").iter().find(|e| e.id == id).cloned();
        Ok(found)
    }

    async fn list_epics_by_project(
        &self,
        project_id: i64,
    ) -> Result<Vec<Epic>, DomainError> {
        let epics: Vec<Epic> = self.epics.lock().expect("epics lock poisoned").iter().filter(|e| e.project_id == project_id).cloned().collect();
        Ok(epics)
    }

    async fn update_epic_status(&self, id: i64, status: EpicStatus) -> Result<(), DomainError> {
        {
            let mut epics = self.epics.lock().expect("epics lock poisoned");
            if let Some(e) = epics.iter_mut().find(|e| e.id == id) {
                e.status = status;
            }
        }
        Ok(())
    }

    async fn create_story(&self, story: &Story) -> Result<i64, DomainError> {
        let mut stories = self.stories.lock().expect("stories lock poisoned");
        let id = (stories.len() as i64) + 1;
        let mut s = story.clone();
        s.id = id;
        stories.push(s);
        Ok(id)
    }

    async fn get_story(&self, id: i64) -> Result<Option<Story>, DomainError> {
        let found = self.stories.lock().expect("stories lock poisoned").iter().find(|s| s.id == id).cloned();
        Ok(found)
    }

    async fn list_stories_by_epic(&self, epic_id: i64) -> Result<Vec<Story>, DomainError> {
        let stories: Vec<Story> = self.stories.lock().expect("stories lock poisoned").iter().filter(|s| s.epic_id == epic_id).cloned().collect();
        Ok(stories)
    }

    async fn update_story_status(&self, id: i64, status: StoryStatus) -> Result<(), DomainError> {
        {
            let mut stories = self.stories.lock().expect("stories lock poisoned");
            if let Some(s) = stories.iter_mut().find(|s| s.id == id) {
                s.status = status;
            }
        }
        Ok(())
    }

    async fn create_user(&self, user: &User) -> Result<i64, DomainError> {
        let mut users = self.users.lock().expect("users lock poisoned");
        let id = (users.len() as i64) + 1;
        let mut u = user.clone();
        u.id = id;
        users.push(u);
        Ok(id)
    }

    async fn get_user(&self, id: i64) -> Result<Option<User>, DomainError> {
        let found = self.users.lock().expect("users lock poisoned").iter().find(|u| u.id == id).cloned();
        Ok(found)
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, DomainError> {
        let found = self.users.lock().expect("users lock poisoned").iter().find(|u| u.email == email).cloned();
        Ok(found)
    }

    async fn list_all_users(&self) -> Result<Vec<User>, DomainError> {
        let all = self.users.lock().expect("users lock poisoned").clone();
        Ok(all)
    }
}
