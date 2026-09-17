// SPDX-License-Identifier: MIT OR Apache-2.0
//! `agileplus-refinery` â€” post-processing pipeline: squash, lint, sign, tag.
//!
//! Audit rec #4.

pub mod config;
pub mod lint;
pub mod sign;
pub mod squash;
pub mod tag;

use std::path::PathBuf;

use agileplus_git::GitVcsAdapter;
use anyhow::{Context, Result};
use config::RefineryConfig;
use lint::{Lint, LintResult};
use sign::{GpgSigner, MockSigner, Signer, SshSigner};
use squash::Squash;
use tag::Tagger;

/// Result of a full refinery run.
#[derive(Debug, Clone)]
pub struct RefineryResult {
    pub squashed: bool,
    pub squash_commit: Option<String>,
    pub lint_result: Option<LintResult>,
    pub signed: bool,
    pub signed_commit: Option<String>,
    pub tagged: bool,
    pub tag: Option<String>,
}

/// Post-processing pipeline.
#[derive(Debug, Clone)]
pub struct Refinery {
    pub config: RefineryConfig,
    pub repo_root: PathBuf,
}

impl Refinery {
    pub fn new(config: RefineryConfig, repo_root: PathBuf) -> Self {
        Self { config, repo_root }
    }

    /// Run the full pipeline.
    ///
    /// 1. Squash source into target
    /// 2. Lint
    /// 3. Sign commit
    /// 4. Tag
    /// 5. Return result
    pub async fn run(&self, source_branch: &str, target_branch: &str) -> Result<RefineryResult> {
        let adapter = GitVcsAdapter::new(self.repo_root.clone());
        let mut result = RefineryResult {
            squashed: false,
            squash_commit: None,
            lint_result: None,
            signed: false,
            signed_commit: None,
            tagged: false,
            tag: None,
        };

        // 1. Squash
        if self.config.squash {
            let squash = Squash::with_root(adapter.clone(), self.repo_root.clone());
            let commit = squash
                .run(
                    source_branch,
                    target_branch,
                    &format!("merge {source_branch} into {target_branch}"),
                )
                .await
                .with_context(|| "squash step failed")?;
            result.squashed = true;
            result.squash_commit = Some(commit.clone());
        }

        let current_commit = result.squash_commit.clone();

        // 2. Lint
        if self.config.lint {
            let lint = Lint;
            let lint_result = lint
                .run(&self.repo_root)
                .await
                .with_context(|| "lint step failed")?;
            result.lint_result = Some(lint_result.clone());
            if !lint_result.overall_pass {
                anyhow::bail!("lint checks failed; stopping pipeline");
            }
        }

        // 3. Sign
        if self.config.sign {
            let commit = current_commit.as_deref().context(
                "sign enabled but no commit produced (squash must be enabled or a commit provided)",
            )?;
            let signer: Box<dyn Signer> = if let Some(key_id) = &self.config.gpg_key_id {
                Box::new(GpgSigner {
                    key_id: key_id.clone(),
                })
            } else if let Some(key_path) = &self.config.ssh_key_path {
                Box::new(SshSigner {
                    key_path: PathBuf::from(key_path),
                })
            } else {
                Box::new(MockSigner)
            };
            let signed_commit = signer
                .sign(&self.repo_root, commit)
                .await
                .with_context(|| "sign step failed")?;
            result.signed = true;
            result.signed_commit = Some(signed_commit);
        }

        // 4. Tag
        if self.config.tag {
            let commit = result
                .signed_commit
                .as_deref()
                .or(current_commit.as_deref())
                .context("tag enabled but no commit to tag")?;
            let tagger = Tagger::new(self.repo_root.clone());
            let tag_name = format!("{source_branch}-refined");
            let tag = tagger
                .create(
                    &tag_name,
                    &format!("Refined from {source_branch} -> {target_branch} @ {commit}"),
                )
                .await
                .with_context(|| "tag step failed")?;
            result.tagged = true;
            result.tag = Some(tag);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_repo(dir: &std::path::Path) {
        let _ = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(dir)
            .output()
            .expect("git init");
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir)
            .output()
            .expect("git config email");
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir)
            .output()
            .expect("git config name");
        // Initial commit
        std::fs::write(dir.join("README.md"), "# init\n").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()
            .expect("git add");
        let _ = Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir)
            .output()
            .expect("git commit");
    }

    fn create_branch(dir: &std::path::Path, name: &str) {
        let _ = Command::new("git")
            .args(["checkout", "-b", name])
            .current_dir(dir)
            .output()
            .expect("git checkout -b");
    }

    fn commit_file(dir: &std::path::Path, filename: &str, content: &str, msg: &str) {
        std::fs::write(dir.join(filename), content).unwrap();
        let _ = Command::new("git")
            .args(["add", filename])
            .current_dir(dir)
            .output()
            .expect("git add");
        let _ = Command::new("git")
            .args(["commit", "-m", msg])
            .current_dir(dir)
            .output()
            .expect("git commit");
    }

    #[tokio::test]
    async fn pipeline_squash_sign_tag_mock() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        init_repo(dir);
        create_branch(dir, "feature");
        commit_file(dir, "a.txt", "hello\n", "feature commit 1");
        commit_file(dir, "b.txt", "world\n", "feature commit 2");

        // Back to main so we can merge feature into it.
        let _ = Command::new("git")
            .args(["checkout", "main"])
            .current_dir(dir)
            .output()
            .expect("checkout main");

        let config = RefineryConfig {
            squash: true,
            sign: true,
            tag: true,
            lint: false,
            gpg_key_id: None,
            ssh_key_path: None,
        };
        let refinery = Refinery::new(config, dir.to_path_buf());
        let result = refinery.run("feature", "main").await.unwrap();

        assert!(result.squashed);
        assert!(result.squash_commit.is_some());
        assert!(result.signed);
        assert!(result.signed_commit.is_some());
        assert!(result.tagged);
        assert!(result.tag.is_some());
        assert_eq!(result.tag.as_ref().unwrap(), "feature-refined");

        // Verify the signed commit message contains [signed]
        let msg = Command::new("git")
            .args(["log", "-1", "--format=%B"])
            .current_dir(dir)
            .output()
            .expect("git log");
        let stdout = String::from_utf8_lossy(&msg.stdout);
        assert!(
            stdout.contains("[signed]"),
            "message should contain [signed]: {stdout}",
        );
    }

    #[tokio::test]
    async fn pipeline_squash_only() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        init_repo(dir);
        create_branch(dir, "feature");
        commit_file(dir, "a.txt", "hello\n", "feature commit");
        let _ = Command::new("git")
            .args(["checkout", "main"])
            .current_dir(dir)
            .output()
            .expect("checkout main");

        let config = RefineryConfig {
            squash: true,
            sign: false,
            tag: false,
            lint: false,
            ..Default::default()
        };
        let refinery = Refinery::new(config, dir.to_path_buf());
        let result = refinery.run("feature", "main").await.unwrap();
        assert!(result.squashed);
        assert!(!result.signed);
        assert!(!result.tagged);
    }

    #[tokio::test]
    async fn pipeline_conflict_detected() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        init_repo(dir);
        commit_file(dir, "shared.txt", "base\n", "base commit");

        create_branch(dir, "feature");
        commit_file(dir, "shared.txt", "feature\n", "feature change");

        let _ = Command::new("git")
            .args(["checkout", "main"])
            .current_dir(dir)
            .output()
            .expect("checkout main");
        commit_file(dir, "shared.txt", "main\n", "main change");

        let config = RefineryConfig {
            squash: true,
            sign: false,
            tag: false,
            lint: false,
            ..Default::default()
        };
        let refinery = Refinery::new(config, dir.to_path_buf());
        let err = refinery.run("feature", "main").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("conflict"), "should report conflict: {msg}");
    }
}
