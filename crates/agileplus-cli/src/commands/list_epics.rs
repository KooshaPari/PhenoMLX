// SPDX-License-Identifier: MIT OR Apache-2.0
//! `list epics [--project <id>]` subcommand.

use anyhow::{Context, Result};
use clap::Args;

use agileplus_domain::ports::StoragePort;

#[derive(Debug, Args)]
pub struct ListEpicsArgs {
    /// Filter to epics belonging to this project id.
    #[arg(long, value_name = "ID")]
    pub project: Option<i64>,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,
}

#[allow(clippy::print_literal)] // table header rows use literal strings for column names
pub async fn run<S: StoragePort>(args: &ListEpicsArgs, storage: &S) -> Result<()> {
    let epics = if let Some(project_id) = args.project {
        storage
            .list_epics_by_project(project_id)
            .await
            .context("listing epics by project")?
    } else {
        // No project filter — collect epics across all projects.
        let projects = storage
            .list_all_projects()
            .await
            .context("listing projects for epic scan")?;
        let mut all = Vec::new();
        for p in &projects {
            let mut es = storage
                .list_epics_by_project(p.id)
                .await
                .context("listing epics")?;
            all.append(&mut es);
        }
        all
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&epics)?);
        return Ok(());
    }

    if epics.is_empty() {
        println!("No epics found.");
        return Ok(());
    }

    println!("{:<6}  {:<6}  {:<12}  {}", "ID", "PROJ", "STATUS", "TITLE");
    println!("{}", "-".repeat(70));
    for e in &epics {
        println!(
            "{:<6}  {:<6}  {:<12}  {}",
            e.id,
            e.project_id,
            e.status.to_string(),
            truncate(&e.title, 40),
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
    fn truncate_keeps_short_titles() {
        assert_eq!(truncate("Epic", 10), "Epic");
    }

    #[test]
    fn truncate_shortens_long_titles() {
        assert_eq!(truncate("Implement everything", 10), "Implement…");
    }
}
