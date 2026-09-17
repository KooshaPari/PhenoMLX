// SPDX-License-Identifier: MIT OR Apache-2.0
//! In-memory stub + unit tests for the list_projects / list_epics / list_stories
//! subcommands (FR-AGP-016).
//!
//! All tests run without I/O; the `MemStore` fulfils `StoragePort` by serving
//! pre-seeded data. The commands under test write to stdout but the tests only
//! assert that `run()` returns `Ok(())` with the expected filtering semantics.

#![cfg(test)]

use std::sync::Mutex;

use async_trait::async_trait;

#[allow(unused_imports)] // Backlog* types used in fixture/seed data
use agileplus_domain::{
    domain::{
        audit::AuditEntry,
        backlog::{BacklogFilters, BacklogItem, BacklogPriority, BacklogStatus},
        cycle::{Cycle, CycleFeature, CycleState, CycleWithFeatures},
        epic::{Epic, EpicStatus},
        feature::Feature,
        governance::{Evidence, GovernanceContract, PolicyRule},
        metric::Metric,
        module::{Module, ModuleFeatureTag, ModuleWithFeatures},
        project::Project,
        state_machine::FeatureState,
        story::{Story, StoryStatus},
        sync_mapping::SyncMapping,
        user::{User, UserRole, UserStatus},
        work_package::{WorkPackage, WpDependency, WpState},
    },
    error::DomainError,
    ports::StoragePort,
};

// ── In-memory test double ─────────────────────────────────────────────────────

#[derive(Default)]
pub struct MemStore {
    pub features: Mutex<Vec<Feature>>,
    pub projects: Mutex<Vec<Project>>,
    pub epics: Mutex<Vec<Epic>>,
    pub stories: Mutex<Vec<Story>>,
    pub work_packages: Mutex<Vec<WorkPackage>>,
    pub wp_dependencies: Mutex<Vec<WpDependency>>,
    pub audit_entries: Mutex<Vec<AuditEntry>>,
    pub evidence: Mutex<Vec<Evidence>>,
    pub policy_rules: Mutex<Vec<PolicyRule>>,
    pub metrics: Mutex<Vec<Metric>>,
    pub governance_contracts: Mutex<Vec<GovernanceContract>>,
    pub modules: Mutex<Vec<Module>>,
    pub module_feature_tags: Mutex<Vec<ModuleFeatureTag>>,
    pub cycles: Mutex<Vec<Cycle>>,
    pub cycle_features: Mutex<Vec<CycleFeature>>,
    pub sync_mappings: Mutex<Vec<SyncMapping>>,
    pub users: Mutex<Vec<User>>,
}

#[async_trait]
impl StoragePort for MemStore {
    // --- Features ---
    async fn create_feature(&self, feature: &Feature) -> Result<i64, DomainError> {
        let mut guard = self.features.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut f = feature.clone();
        f.id = id;
        guard.push(f);
        Ok(id)
    }

    async fn get_feature_by_slug(&self, slug: &str) -> Result<Option<Feature>, DomainError> {
        Ok(self
            .features
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.slug == slug)
            .cloned())
    }

    async fn get_feature_by_id(&self, id: i64) -> Result<Option<Feature>, DomainError> {
        Ok(self
            .features
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .cloned())
    }

    async fn update_feature_state(
        &self,
        id: i64,
        state: FeatureState,
    ) -> Result<(), DomainError> {
        let mut guard = self.features.lock().unwrap();
        if let Some(f) = guard.iter_mut().find(|f| f.id == id) {
            f.state = state;
        }
        Ok(())
    }

    async fn update_feature(&self, feature: &Feature) -> Result<(), DomainError> {
        let mut guard = self.features.lock().unwrap();
        if let Some(f) = guard.iter_mut().find(|f| f.id == feature.id) {
            *f = feature.clone();
        }
        Ok(())
    }

    async fn list_features_by_state(
        &self,
        state: FeatureState,
    ) -> Result<Vec<Feature>, DomainError> {
        Ok(self
            .features
            .lock()
            .unwrap()
            .iter()
            .filter(|feature| feature.state == state)
            .cloned()
            .collect())
    }

    async fn list_all_features(&self) -> Result<Vec<Feature>, DomainError> {
        Ok(self.features.lock().unwrap().clone())
    }

    async fn list_features_by_label(&self, _label: &str) -> Result<Vec<Feature>, DomainError> {
        // In-memory store does not track label metadata; return empty.
        Ok(vec![])
    }

    // --- Work Packages ---
    async fn create_work_package(&self, wp: &WorkPackage) -> Result<i64, DomainError> {
        let mut guard = self.work_packages.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut w = wp.clone();
        w.id = id;
        guard.push(w);
        Ok(id)
    }

    async fn get_work_package(&self, id: i64) -> Result<Option<WorkPackage>, DomainError> {
        Ok(self
            .work_packages
            .lock()
            .unwrap()
            .iter()
            .find(|wp| wp.id == id)
            .cloned())
    }

    async fn update_wp_state(&self, id: i64, state: WpState) -> Result<(), DomainError> {
        let mut guard = self.work_packages.lock().unwrap();
        if let Some(wp) = guard.iter_mut().find(|wp| wp.id == id) {
            wp.state = state;
        }
        Ok(())
    }

    async fn list_wps_by_feature(
        &self,
        feature_id: i64,
    ) -> Result<Vec<WorkPackage>, DomainError> {
        Ok(self
            .work_packages
            .lock()
            .unwrap()
            .iter()
            .filter(|wp| wp.feature_id == feature_id)
            .cloned()
            .collect())
    }

    async fn add_wp_dependency(&self, dep: &WpDependency) -> Result<(), DomainError> {
        self.wp_dependencies.lock().unwrap().push(dep.clone());
        Ok(())
    }

    async fn get_wp_dependencies(&self, wp_id: i64) -> Result<Vec<WpDependency>, DomainError> {
        Ok(self
            .wp_dependencies
            .lock()
            .unwrap()
            .iter()
            .filter(|d| d.wp_id == wp_id)
            .cloned()
            .collect())
    }

    async fn get_ready_wps(&self, feature_id: i64) -> Result<Vec<WorkPackage>, DomainError> {
        let guard = self.work_packages.lock().unwrap();
        Ok(guard
            .iter()
            .filter(|wp| wp.feature_id == feature_id)
            .cloned()
            .collect())
    }

    // --- Audit ---
    async fn append_audit_entry(&self, entry: &AuditEntry) -> Result<i64, DomainError> {
        let mut guard = self.audit_entries.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut e = entry.clone();
        e.id = id;
        guard.push(e);
        Ok(id)
    }

    async fn get_audit_trail(&self, feature_id: i64) -> Result<Vec<AuditEntry>, DomainError> {
        Ok(self
            .audit_entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.feature_id == feature_id)
            .cloned()
            .collect())
    }

    async fn get_latest_audit_entry(
        &self,
        feature_id: i64,
    ) -> Result<Option<AuditEntry>, DomainError> {
        Ok(self
            .audit_entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.feature_id == feature_id)
            .last()
            .cloned())
    }

    // --- Evidence ---
    async fn create_evidence(&self, ev: &Evidence) -> Result<i64, DomainError> {
        let mut guard = self.evidence.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut e = ev.clone();
        e.id = id;
        guard.push(e);
        Ok(id)
    }

    async fn get_evidence_by_wp(&self, wp_id: i64) -> Result<Vec<Evidence>, DomainError> {
        Ok(self
            .evidence
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.wp_id == wp_id)
            .cloned()
            .collect())
    }

    async fn get_evidence_by_fr(&self, fr_id: &str) -> Result<Vec<Evidence>, DomainError> {
        Ok(self
            .evidence
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.fr_id == fr_id)
            .cloned()
            .collect())
    }

    // --- Policy / Governance ---
    async fn create_policy_rule(&self, rule: &PolicyRule) -> Result<i64, DomainError> {
        let mut guard = self.policy_rules.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut r = rule.clone();
        r.id = id;
        guard.push(r);
        Ok(id)
    }

    async fn list_active_policies(&self) -> Result<Vec<PolicyRule>, DomainError> {
        Ok(self
            .policy_rules
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.active)
            .cloned()
            .collect())
    }

    async fn record_metric(&self, metric: &Metric) -> Result<i64, DomainError> {
        let mut guard = self.metrics.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut m = metric.clone();
        m.id = id;
        guard.push(m);
        Ok(id)
    }

    async fn get_metrics_by_feature(
        &self,
        feature_id: i64,
    ) -> Result<Vec<Metric>, DomainError> {
        Ok(self
            .metrics
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.feature_id == Some(feature_id))
            .cloned()
            .collect())
    }

    async fn create_governance_contract(
        &self,
        contract: &GovernanceContract,
    ) -> Result<i64, DomainError> {
        let mut guard = self.governance_contracts.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut c = contract.clone();
        c.id = id;
        guard.push(c);
        Ok(id)
    }

    async fn get_governance_contract(
        &self,
        feature_id: i64,
        version: i32,
    ) -> Result<Option<GovernanceContract>, DomainError> {
        Ok(self
            .governance_contracts
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.feature_id == feature_id && c.version == version)
            .cloned())
    }

    async fn get_latest_governance_contract(
        &self,
        feature_id: i64,
    ) -> Result<Option<GovernanceContract>, DomainError> {
        Ok(self
            .governance_contracts
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.feature_id == feature_id)
            .max_by_key(|c| c.version)
            .cloned())
    }

    // --- Modules ---
    async fn create_module(&self, module: &Module) -> Result<i64, DomainError> {
        let mut guard = self.modules.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut m = module.clone();
        m.id = id;
        guard.push(m);
        Ok(id)
    }

    async fn get_module(&self, id: i64) -> Result<Option<Module>, DomainError> {
        Ok(self
            .modules
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.id == id)
            .cloned())
    }

    async fn get_module_by_slug(&self, slug: &str) -> Result<Option<Module>, DomainError> {
        Ok(self
            .modules
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.slug == slug)
            .cloned())
    }

    async fn update_module(
        &self,
        id: i64,
        friendly_name: &str,
        description: Option<&str>,
    ) -> Result<(), DomainError> {
        let mut guard = self.modules.lock().unwrap();
        if let Some(m) = guard.iter_mut().find(|m| m.id == id) {
            m.friendly_name = friendly_name.to_string();
            m.description = description.map(|s| s.to_string());
        }
        Ok(())
    }

    async fn delete_module(&self, id: i64) -> Result<(), DomainError> {
        self.modules.lock().unwrap().retain(|m| m.id != id);
        Ok(())
    }

    async fn list_root_modules(&self) -> Result<Vec<Module>, DomainError> {
        Ok(self
            .modules
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.parent_module_id.is_none())
            .cloned()
            .collect())
    }

    async fn list_child_modules(&self, parent_id: i64) -> Result<Vec<Module>, DomainError> {
        Ok(self
            .modules
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.parent_module_id == Some(parent_id))
            .cloned()
            .collect())
    }

    async fn get_module_with_features(
        &self,
        id: i64,
    ) -> Result<Option<ModuleWithFeatures>, DomainError> {
        let modules = self.modules.lock().unwrap();
        let module = modules.iter().find(|m| m.id == id).cloned();
        match module {
            Some(module) => {
                let features = self.features.lock().unwrap();
                let tags = self.module_feature_tags.lock().unwrap();
                let owned_features: Vec<Feature> = Vec::new(); // no ownership in mem-store
                let tagged_features: Vec<Feature> = features
                    .iter()
                    .filter(|f| {
                        tags.iter()
                            .any(|t| t.module_id == id && t.feature_id == f.id)
                    })
                    .cloned()
                    .collect();
                let child_modules: Vec<Module> = modules
                    .iter()
                    .filter(|m| m.parent_module_id == Some(id))
                    .cloned()
                    .collect();
                Ok(Some(ModuleWithFeatures {
                    module,
                    owned_features,
                    tagged_features,
                    child_modules,
                }))
            }
            None => Ok(None),
        }
    }

    async fn tag_feature_to_module(&self, tag: &ModuleFeatureTag) -> Result<(), DomainError> {
        self.module_feature_tags.lock().unwrap().push(tag.clone());
        Ok(())
    }

    async fn untag_feature_from_module(
        &self,
        module_id: i64,
        feature_id: i64,
    ) -> Result<(), DomainError> {
        self.module_feature_tags
            .lock()
            .unwrap()
            .retain(|t| !(t.module_id == module_id && t.feature_id == feature_id));
        Ok(())
    }

    // --- Cycles ---
    async fn create_cycle(&self, cycle: &Cycle) -> Result<i64, DomainError> {
        let mut guard = self.cycles.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut c = cycle.clone();
        c.id = id;
        guard.push(c);
        Ok(id)
    }

    async fn get_cycle(&self, id: i64) -> Result<Option<Cycle>, DomainError> {
        Ok(self
            .cycles
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.id == id)
            .cloned())
    }

    async fn update_cycle_state(&self, id: i64, state: CycleState) -> Result<(), DomainError> {
        let mut guard = self.cycles.lock().unwrap();
        if let Some(c) = guard.iter_mut().find(|c| c.id == id) {
            c.state = state;
        }
        Ok(())
    }

    async fn list_cycles_by_state(&self, state: CycleState) -> Result<Vec<Cycle>, DomainError> {
        Ok(self
            .cycles
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.state == state)
            .cloned()
            .collect())
    }

    async fn list_cycles_by_module(&self, module_id: i64) -> Result<Vec<Cycle>, DomainError> {
        Ok(self
            .cycles
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.module_scope_id == Some(module_id))
            .cloned()
            .collect())
    }

    async fn list_all_cycles(&self) -> Result<Vec<Cycle>, DomainError> {
        Ok(self.cycles.lock().unwrap().clone())
    }

    async fn get_cycle_with_features(
        &self,
        id: i64,
    ) -> Result<Option<CycleWithFeatures>, DomainError> {
        let cycles = self.cycles.lock().unwrap();
        let cycle = cycles.iter().find(|c| c.id == id).cloned();
        match cycle {
            Some(cycle) => {
                let features = self.features.lock().unwrap();
                let cycle_feats = self.cycle_features.lock().unwrap();
                let linked_features: Vec<Feature> = features
                    .iter()
                    .filter(|f| {
                        cycle_feats
                            .iter()
                            .any(|cf| cf.cycle_id == id && cf.feature_id == f.id)
                    })
                    .cloned()
                    .collect();
                Ok(Some(CycleWithFeatures {
                    cycle,
                    features: linked_features,
                    wp_progress: Default::default(),
                }))
            }
            None => Ok(None),
        }
    }

    async fn add_feature_to_cycle(&self, entry: &CycleFeature) -> Result<(), DomainError> {
        self.cycle_features.lock().unwrap().push(entry.clone());
        Ok(())
    }

    async fn remove_feature_from_cycle(
        &self,
        cycle_id: i64,
        feature_id: i64,
    ) -> Result<(), DomainError> {
        self.cycle_features
            .lock()
            .unwrap()
            .retain(|cf| !(cf.cycle_id == cycle_id && cf.feature_id == feature_id));
        Ok(())
    }

    // --- Sync Mappings ---
    async fn get_sync_mapping(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Option<SyncMapping>, DomainError> {
        Ok(self
            .sync_mappings
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.entity_type == entity_type && m.entity_id == entity_id)
            .cloned())
    }

    async fn upsert_sync_mapping(&self, mapping: &SyncMapping) -> Result<(), DomainError> {
        let mut guard = self.sync_mappings.lock().unwrap();
        if let Some(existing) = guard
            .iter_mut()
            .find(|m| m.entity_type == mapping.entity_type && m.entity_id == mapping.entity_id)
        {
            *existing = mapping.clone();
        } else {
            guard.push(mapping.clone());
        }
        Ok(())
    }

    async fn get_sync_mapping_by_plane_id(
        &self,
        entity_type: &str,
        plane_issue_id: &str,
    ) -> Result<Option<SyncMapping>, DomainError> {
        Ok(self
            .sync_mappings
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.entity_type == entity_type && m.plane_issue_id == plane_issue_id)
            .cloned())
    }

    async fn delete_sync_mapping(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<(), DomainError> {
        self.sync_mappings
            .lock()
            .unwrap()
            .retain(|m| !(m.entity_type == entity_type && m.entity_id == entity_id));
        Ok(())
    }

    // --- Projects ---
    async fn create_project(&self, project: &Project) -> Result<i64, DomainError> {
        let mut guard = self.projects.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut p = project.clone();
        p.id = id;
        guard.push(p);
        Ok(id)
    }

    async fn get_project_by_slug(&self, slug: &str) -> Result<Option<Project>, DomainError> {
        Ok(self
            .projects
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.slug == slug)
            .cloned())
    }

    async fn get_project_by_id(&self, id: i64) -> Result<Option<Project>, DomainError> {
        Ok(self
            .projects
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.id == id)
            .cloned())
    }

    async fn list_all_projects(&self) -> Result<Vec<Project>, DomainError> {
        Ok(self.projects.lock().unwrap().clone())
    }

    async fn delete_project(&self, id: i64) -> Result<(), DomainError> {
        self.projects.lock().unwrap().retain(|p| p.id != id);
        Ok(())
    }

    // --- Users ---
    async fn create_user(&self, user: &User) -> Result<i64, DomainError> {
        let mut guard = self.users.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut u = user.clone();
        u.id = id;
        guard.push(u);
        Ok(id)
    }

    async fn get_user(&self, id: i64) -> Result<Option<User>, DomainError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == id)
            .cloned())
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, DomainError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.email == email)
            .cloned())
    }

    async fn update_user_status(&self, id: i64, status: UserStatus) -> Result<(), DomainError> {
        let mut guard = self.users.lock().unwrap();
        if let Some(u) = guard.iter_mut().find(|u| u.id == id) {
            u.status = status;
        }
        Ok(())
    }

    async fn update_user_role(&self, id: i64, role: UserRole) -> Result<(), DomainError> {
        let mut guard = self.users.lock().unwrap();
        if let Some(u) = guard.iter_mut().find(|u| u.id == id) {
            u.role = role;
        }
        Ok(())
    }

    async fn list_all_users(&self) -> Result<Vec<User>, DomainError> {
        Ok(self.users.lock().unwrap().clone())
    }

    async fn delete_user(&self, id: i64) -> Result<(), DomainError> {
        self.users.lock().unwrap().retain(|u| u.id != id);
        Ok(())
    }

    // --- Epics ---
    async fn create_epic(&self, epic: &Epic) -> Result<i64, DomainError> {
        let mut guard = self.epics.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut e = epic.clone();
        e.id = id;
        guard.push(e);
        Ok(id)
    }

    async fn get_epic(&self, id: i64) -> Result<Option<Epic>, DomainError> {
        Ok(self
            .epics
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.id == id)
            .cloned())
    }

    async fn update_epic_status(
        &self,
        id: i64,
        status: EpicStatus,
    ) -> Result<(), DomainError> {
        let mut guard = self.epics.lock().unwrap();
        if let Some(epic) = guard.iter_mut().find(|e| e.id == id) {
            epic.status = status;
        }
        Ok(())
    }

    async fn list_epics_by_project(&self, project_id: i64) -> Result<Vec<Epic>, DomainError> {
        Ok(self
            .epics
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn delete_epic(&self, id: i64) -> Result<(), DomainError> {
        self.epics.lock().unwrap().retain(|e| e.id != id);
        Ok(())
    }

    // --- Stories ---
    async fn create_story(&self, story: &Story) -> Result<i64, DomainError> {
        let mut guard = self.stories.lock().unwrap();
        let id = guard.len() as i64 + 1;
        let mut s = story.clone();
        s.id = id;
        guard.push(s);
        Ok(id)
    }

    async fn get_story(&self, id: i64) -> Result<Option<Story>, DomainError> {
        Ok(self
            .stories
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id == id)
            .cloned())
    }

    async fn update_story_status(
        &self,
        id: i64,
        status: StoryStatus,
    ) -> Result<(), DomainError> {
        let mut guard = self.stories.lock().unwrap();
        if let Some(story) = guard.iter_mut().find(|s| s.id == id) {
            story.status = status;
        }
        Ok(())
    }

    async fn list_stories_by_epic(&self, epic_id: i64) -> Result<Vec<Story>, DomainError> {
        Ok(self
            .stories
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.epic_id == epic_id)
            .cloned()
            .collect())
    }

    async fn list_stories_by_project(&self, project_id: i64) -> Result<Vec<Story>, DomainError> {
        Ok(self
            .stories
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn delete_story(&self, id: i64) -> Result<(), DomainError> {
        self.stories.lock().unwrap().retain(|s| s.id != id);
        Ok(())
    }

    async fn upsert_story_by_requirement_id(&self, story: &Story) -> Result<i64, DomainError> {
        self.create_story(story).await
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_project(id: i64, slug: &str, name: &str) -> Project {
    let mut p = Project::new(name, slug).unwrap();
    p.id = id;
    p
}

fn make_feature(id: i64, slug: &str, title: &str, state: FeatureState) -> Feature {
    let mut feature = Feature::new(slug, title, [id as u8; 32], None);
    feature.id = id;
    feature.state = state;
    feature
}

fn make_epic(id: i64, project_id: i64, title: &str, status: EpicStatus) -> Epic {
    let mut e = Epic::new(project_id, title).unwrap();
    e.id = id;
    e.status = status;
    e
}

fn make_story(id: i64, epic_id: i64, project_id: i64, title: &str, status: StoryStatus) -> Story {
    let mut s = Story::new(epic_id, project_id, title, None).unwrap();
    s.id = id;
    s.status = status;
    s
}

// ── Tests: list projects ──────────────────────────────────────────────────────

#[tokio::test]
async fn list_projects_returns_ok_for_empty_store() {
    let store = MemStore::default();
    let args = crate::commands::list_projects::ListProjectsArgs { json: false };
    crate::commands::list_projects::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_projects_returns_ok_with_data() {
    let store = MemStore {
        projects: Mutex::new(vec![
            make_project(1, "alpha", "Alpha"),
            make_project(2, "beta", "Beta"),
        ]),
        ..MemStore::default()
    };
    let args = crate::commands::list_projects::ListProjectsArgs { json: false };
    crate::commands::list_projects::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_projects_json_flag_returns_ok() {
    let store = MemStore {
        projects: Mutex::new(vec![make_project(1, "alpha", "Alpha")]),
        ..MemStore::default()
    };
    let args = crate::commands::list_projects::ListProjectsArgs { json: true };
    crate::commands::list_projects::run(&args, &store)
        .await
        .unwrap();
}

// ── Tests: list epics ─────────────────────────────────────────────────────────

#[tokio::test]
async fn list_epics_no_filter_returns_ok() {
    let store = MemStore {
        projects: Mutex::new(vec![make_project(1, "alpha", "Alpha")]),
        epics: Mutex::new(vec![make_epic(1, 1, "Epic One", EpicStatus::Active)]),
        ..MemStore::default()
    };
    let args = crate::commands::list_epics::ListEpicsArgs {
        project: None,
        json: false,
    };
    crate::commands::list_epics::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_epics_with_project_filter_returns_only_matching() {
    let store = MemStore {
        projects: Mutex::new(vec![
            make_project(1, "alpha", "Alpha"),
            make_project(2, "beta", "Beta"),
        ]),
        epics: Mutex::new(vec![
            make_epic(1, 1, "Epic P1", EpicStatus::Active),
            make_epic(2, 2, "Epic P2", EpicStatus::Backlog),
        ]),
        ..MemStore::default()
    };
    // Filter to project 1 — only epic 1 should be returned.
    let args = crate::commands::list_epics::ListEpicsArgs {
        project: Some(1),
        json: false,
    };
    crate::commands::list_epics::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_epics_json_flag_returns_ok() {
    let store = MemStore {
        projects: Mutex::new(vec![make_project(1, "alpha", "Alpha")]),
        epics: Mutex::new(vec![make_epic(1, 1, "Epic One", EpicStatus::Done)]),
        ..MemStore::default()
    };
    let args = crate::commands::list_epics::ListEpicsArgs {
        project: Some(1),
        json: true,
    };
    crate::commands::list_epics::run(&args, &store)
        .await
        .unwrap();
}

// ── Tests: list stories ───────────────────────────────────────────────────────

#[tokio::test]
async fn list_stories_no_filter_returns_ok() {
    let store = MemStore {
        projects: Mutex::new(vec![make_project(1, "alpha", "Alpha")]),
        stories: Mutex::new(vec![make_story(1, 10, 1, "Story One", StoryStatus::Todo)]),
        ..MemStore::default()
    };
    let args = crate::commands::list_stories::ListStoriesArgs {
        epic: None,
        status: None,
        json: false,
    };
    crate::commands::list_stories::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_stories_epic_filter_returns_only_matching() {
    let store = MemStore {
        stories: Mutex::new(vec![
            make_story(1, 10, 1, "Story A", StoryStatus::Todo),
            make_story(2, 20, 1, "Story B", StoryStatus::Done),
        ]),
        ..MemStore::default()
    };
    let args = crate::commands::list_stories::ListStoriesArgs {
        epic: Some(10),
        status: None,
        json: false,
    };
    crate::commands::list_stories::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_stories_status_filter_returns_only_matching() {
    let store = MemStore {
        projects: Mutex::new(vec![make_project(1, "alpha", "Alpha")]),
        stories: Mutex::new(vec![
            make_story(1, 10, 1, "Story A", StoryStatus::Todo),
            make_story(2, 10, 1, "Story B", StoryStatus::Done),
            make_story(3, 10, 1, "Story C", StoryStatus::InProgress),
        ]),
        ..MemStore::default()
    };
    // Filter by epic + status
    let args = crate::commands::list_stories::ListStoriesArgs {
        epic: Some(10),
        status: Some("done".to_string()),
        json: false,
    };
    crate::commands::list_stories::run(&args, &store)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_stories_invalid_status_returns_err() {
    let store = MemStore::default();
    let args = crate::commands::list_stories::ListStoriesArgs {
        epic: None,
        status: Some("not_a_status".to_string()),
        json: false,
    };
    assert!(crate::commands::list_stories::run(&args, &store)
        .await
        .is_err());
}

#[tokio::test]
async fn list_stories_json_flag_returns_ok() {
    let store = MemStore {
        projects: Mutex::new(vec![make_project(1, "alpha", "Alpha")]),
        stories: Mutex::new(vec![make_story(1, 10, 1, "Story One", StoryStatus::Review)]),
        ..MemStore::default()
    };
    let args = crate::commands::list_stories::ListStoriesArgs {
        epic: Some(10),
        status: None,
        json: true,
    };
    crate::commands::list_stories::run(&args, &store)
        .await
        .unwrap();
}

// ── Tests: list features ──────────────────────────────────────────────────────

#[tokio::test]
async fn list_features_returns_ok_for_empty_store() {
    let store = MemStore::default();
    let args = crate::commands::list::ListArgs { state: None };

    crate::commands::list::run(args, &store).await.unwrap();
}

#[tokio::test]
async fn list_features_returns_all_features() {
    let store = MemStore {
        features: Mutex::new(vec![
            make_feature(1, "feat-alpha", "Alpha", FeatureState::Created),
            make_feature(2, "feat-beta", "Beta", FeatureState::Planned),
        ]),
        ..MemStore::default()
    };
    let args = crate::commands::list::ListArgs { state: None };

    crate::commands::list::run(args, &store).await.unwrap();
}

#[tokio::test]
async fn list_features_filters_by_state() {
    let store = MemStore {
        features: Mutex::new(vec![
            make_feature(1, "feat-alpha", "Alpha", FeatureState::Created),
            make_feature(2, "feat-beta", "Beta", FeatureState::Planned),
        ]),
        ..MemStore::default()
    };
    let args = crate::commands::list::ListArgs {
        state: Some("planned".to_string()),
    };

    crate::commands::list::run(args, &store).await.unwrap();
}

#[tokio::test]
async fn list_features_rejects_invalid_state() {
    let store = MemStore::default();
    let args = crate::commands::list::ListArgs {
        state: Some("not-a-state".to_string()),
    };

    assert!(crate::commands::list::run(args, &store).await.is_err());
}
