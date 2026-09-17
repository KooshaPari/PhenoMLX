// SPDX-License-Identifier: MIT OR Apache-2.0
//! `list projects` subcommand — read all projects from the repository port.

use anyhow::{Context, Result};
use clap::Args;

use agileplus_domain::ports::StoragePort;

#[derive(Debug, Args)]
pub struct ListProjectsArgs {
    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,
}

#[allow(clippy::print_literal)] // table header rows use literal strings for column names
pub async fn run<S: StoragePort>(args: &ListProjectsArgs, storage: &S) -> Result<()> {
    let projects = storage
        .list_all_projects()
        .await
        .context("listing projects")?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&projects)?);
        return Ok(());
    }

    if projects.is_empty() {
        println!("No projects found.");
        return Ok(());
    }

    println!("{:<6}  {:<24}  {}", "ID", "SLUG", "NAME");
    println!("{}", "-".repeat(60));
    for p in &projects {
        println!("{:<6}  {:<24}  {}", p.id, truncate(&p.slug, 24), p.name);
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
    fn truncate_keeps_values_within_limit() {
        assert_eq!(truncate("project", 12), "project");
    }

    #[test]
    fn truncate_shortens_values_past_limit() {
        assert_eq!(truncate("project-alpha", 8), "project…");
    }
}
