// SPDX-License-Identifier: MIT OR Apache-2.0
//! `list stories [--epic <id>] [--status <s>]` subcommand.

use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use clap::Args;

use agileplus_domain::{domain::story::StoryStatus, ports::StoragePort};

#[derive(Debug, Args)]
pub struct ListStoriesArgs {
    /// Filter to stories belonging to this epic id.
    #[arg(long, value_name = "ID")]
    pub epic: Option<i64>,

    /// Filter by status (todo, in_progress, review, done, blocked, cancelled).
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,
}

#[allow(clippy::print_literal)] // table header uses literal strings
pub async fn run<S: StoragePort>(args: &ListStoriesArgs, storage: &S) -> Result<()> {
    // Parse status filter eagerly so we fail fast on a bad value.
    let status_filter: Option<StoryStatus> = args
        .status
        .as_deref()
        .map(|s| StoryStatus::from_str(s).map_err(|e| anyhow!("{e}")))
        .transpose()?;

    let mut stories = if let Some(epic_id) = args.epic {
        storage
            .list_stories_by_epic(epic_id)
            .await
            .context("listing stories by epic")?
    } else {
        // No epic filter — collect across all projects.
        let projects = storage
            .list_all_projects()
            .await
            .context("listing projects for story scan")?;
        let mut all = Vec::new();
        for p in &projects {
            let mut ss = storage
                .list_stories_by_project(p.id)
                .await
                .context("listing stories by project")?;
            all.append(&mut ss);
        }
        all
    };

    if let Some(filter) = status_filter {
        stories.retain(|s| s.status == filter);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&stories)?);
        return Ok(());
    }

    if stories.is_empty() {
        println!("No stories found.");
        return Ok(());
    }

    println!("{:<6}  {:<6}  {:<12}  {}", "ID", "EPIC", "STATUS", "TITLE");
    println!("{}", "-".repeat(70));
    for s in &stories {
        println!(
            "{:<6}  {:<6}  {:<12}  {}",
            s.id,
            s.epic_id,
            s.status.to_string(),
            truncate(&s.title, 40),
        );
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_story_titles() {
        assert_eq!(truncate("Story", 10), "Story");
    }

    #[test]
    fn truncate_shortens_long_story_titles() {
        assert_eq!(truncate("Write focused coverage", 12), "Write focus…");
    }
}
