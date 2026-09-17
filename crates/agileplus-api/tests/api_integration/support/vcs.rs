use std::path::{Path, PathBuf};

use agileplus_domain::error::DomainError;
use agileplus_domain::ports::vcs::{
    BranchInfo, ConflictInfo, FeatureArtifacts, MergeResult, VcsPort, WorktreeInfo,
};
use async_trait::async_trait;

#[derive(Clone)]
pub(crate) struct MockVcs;

#[async_trait]
impl VcsPort for MockVcs {
    async fn create_worktree(
        &self,
        _fs: &str,
        _wp: &str,
    ) -> Result<PathBuf, DomainError> {
        Ok(PathBuf::from("/tmp/worktree"))
    }

    async fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, DomainError> {
        Ok(vec![])
    }

    async fn cleanup_worktree(&self, _p: &Path) -> Result<(), DomainError> {
        Ok(())
    }

    async fn create_branch(
        &self,
        _b: &str,
        _base: &str,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn list_branches(
        &self,
        _pattern: Option<&str>,
        _remote: bool,
    ) -> Result<Vec<BranchInfo>, DomainError> {
        Ok(vec![])
    }

    async fn delete_branch(
        &self,
        _branch_name: &str,
        _force: bool,
        _remote: Option<&str>,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn checkout_branch(&self, _b: &str) -> Result<(), DomainError> {
        Ok(())
    }

    async fn merge_to_target(
        &self,
        _s: &str,
        _t: &str,
    ) -> Result<MergeResult, DomainError> {
        Ok(MergeResult {
            success: true,
            conflicts: vec![],
            merged_commit: None,
            commit: None,
            message: None,
        })
    }

    async fn detect_conflicts(
        &self,
        _s: &str,
        _t: &str,
    ) -> Result<Vec<ConflictInfo>, DomainError> {
        Ok(vec![])
    }

    async fn read_artifact(
        &self,
        _fs: &str,
        _p: &str,
    ) -> Result<String, DomainError> {
        Ok(String::new())
    }

    async fn write_artifact(
        &self,
        _fs: &str,
        _p: &str,
        _c: &str,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    async fn artifact_exists(
        &self,
        _fs: &str,
        _p: &str,
    ) -> Result<bool, DomainError> {
        Ok(false)
    }

    async fn scan_feature_artifacts(&self, _fs: &str) -> Result<FeatureArtifacts, DomainError> {
        Ok(FeatureArtifacts {
            spec: None,
            research: None,
            plan: None,
            other: vec![],
            meta_json: None,
            audit_chain: None,
            evidence_paths: vec![],
        })
    }
}
