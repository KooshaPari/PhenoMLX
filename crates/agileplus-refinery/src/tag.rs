// SPDX-License-Identifier: MIT OR Apache-2.0
//! Annotated git tag creation.

use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Tagger — creates annotated git tags.
#[derive(Debug, Clone)]
pub struct Tagger {
    repo_root: std::path::PathBuf,
}

impl Tagger {
    pub fn new(repo_root: std::path::PathBuf) -> Self {
        Self { repo_root }
    }

    /// Create an annotated tag `name` with message `message`.
    pub async fn create(&self, name: &str, message: &str) -> Result<String> {
        let output = tokio::task::spawn_blocking({
            let repo_root = self.repo_root.clone();
            let name = name.to_string();
            let message = message.to_string();
            move || {
                Command::new("git")
                    .args(["tag", "-a", &name, "-m", &message])
                    .current_dir(&repo_root)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .with_context(|| format!("git tag -a {name} failed"))
            }
        })
        .await
        .context("spawn_blocking failed")??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git tag failed: {}", stderr.trim());
        }

        // Return the tag ref (e.g. v1.0.0)
        Ok(name.to_string())
    }
}
