// SPDX-License-Identifier: MIT OR Apache-2.0
//! Repo introspection: detect git state, mangled directories, no-git
//! directories. Produces a `RepoInfo` snapshot suitable for triage
//! classification of the local working tree.
//!
//! Traceability: FR-AGP-020 (repo introspection)

use serde::{Deserialize, Serialize};
use std::path::Path;

/// State of a directory as a candidate repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoState {
    /// Has a valid `.git/` directory with HEAD, branches, remotes.
    Git,
    /// Has `.git/` but it's corrupt or in an unexpected state (mangled).
    MangledGit,
    /// No `.git/` at all (plain directory, subproject of a parent repo, or fresh clone).
    NoGit,
}

/// One remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

/// Snapshot of a repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: String,
    pub state: RepoState,
    pub current_branch: Option<String>,
    pub branches: Vec<String>,
    pub worktrees: Vec<String>,
    pub remotes: Vec<RemoteInfo>,
    /// 0..=100. `Git`=100, `MangledGit`=50, `NoGit`=30, `NoGit`+source markers=70
    /// (treated as a subproject of a parent repo).
    pub hygiene_score: u8,
}

/// Inspect a directory and produce a `RepoInfo` snapshot.
pub fn inspect_repo(path: &Path) -> RepoInfo {
    let path_str = path.to_string_lossy().into_owned();
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        let hygiene = if has_source_files(path) { 70 } else { 30 };
        return RepoInfo {
            path: path_str,
            state: RepoState::NoGit,
            current_branch: None,
            branches: vec![],
            worktrees: vec![],
            remotes: vec![],
            hygiene_score: hygiene,
        };
    }
    let head = git_dir.join("HEAD");
    if !head.exists() {
        return RepoInfo {
            path: path_str,
            state: RepoState::MangledGit,
            hygiene_score: 50,
            current_branch: None,
            branches: vec![],
            worktrees: vec![],
            remotes: vec![],
        };
    }
    let head_content = std::fs::read_to_string(&head).unwrap_or_default();
    let current_branch = head_content
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("ref: refs/heads/"))
        .map(String::from);
    let branches = read_branches(&git_dir);
    let worktrees = read_worktrees(&git_dir);
    let remotes = read_remotes(&git_dir);
    RepoInfo {
        path: path_str,
        state: RepoState::Git,
        hygiene_score: 100,
        current_branch,
        branches,
        worktrees,
        remotes,
    }
}

fn read_branches(git_dir: &Path) -> Vec<String> {
    let heads = git_dir.join("refs").join("heads");
    if !heads.exists() {
        return vec![];
    }
    walk_dir_files(&heads)
        .into_iter()
        .filter_map(|p| {
            p.strip_prefix(&heads)
                .ok()
                .map(|r| r.to_string_lossy().into_owned())
        })
        .collect()
}

fn read_worktrees(git_dir: &Path) -> Vec<String> {
    let wt = git_dir.join("worktrees");
    if !wt.exists() {
        return vec![];
    }
    std::fs::read_dir(&wt)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn read_remotes(git_dir: &Path) -> Vec<RemoteInfo> {
    let cfg = git_dir.join("config");
    let raw = std::fs::read_to_string(&cfg).unwrap_or_default();
    let mut out = vec![];
    let mut current_name: Option<String> = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.contains("remote") {
            // [remote "origin"]
            let name = trimmed.split('"').nth(1).unwrap_or("").to_string();
            if !name.is_empty() {
                current_name = Some(name);
            }
        } else if let Some(name) = &current_name {
            if let Some(url) = trimmed.strip_prefix("url = ") {
                out.push(RemoteInfo {
                    name: name.clone(),
                    url: url.to_string(),
                });
                current_name = None;
            }
        }
    }
    out
}

fn walk_dir_files(p: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                out.extend(walk_dir_files(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

fn has_source_files(p: &Path) -> bool {
    for marker in &[
        "src", "lib", "pkg", "cmd", "crates", "backend", "frontend", "app",
    ] {
        if p.join(marker).is_dir() {
            return true;
        }
    }
    false
}
