// SPDX-License-Identifier: MIT OR Apache-2.0
//! `seed-requirements` CLI subcommand.
//!
//! Ingests the six FR/NFR catalogs (AgilePlus, Tracera, phenotype-voxel, Authvault,
//! PhenoMCP, PhenoObservability) as Epics + Stories into an AgilePlus SQLite database,
//! each cross-referencing its Tracera Requirement ID.  Idempotent â€” safe to run multiple times.

use std::path::PathBuf;

use clap::Args;

use agileplus_sqlite::seed::{seed_requirements, Initiative};

/// Embedded catalog markdown files â€” bundled at compile time.
/// Paths are relative to this source file:
///   crates/agileplus-cli/src/commands/seed_requirements.rs
///   â†’ up 4 dirs â†’ workspace root â†’ docs/requirements/
const AGILEPLUS_CATALOG: &str = include_str!("../../../../docs/requirements/agileplus-frnfr.md");
const TRACERA_CATALOG: &str = include_str!("../../../../docs/requirements/tracera-frnfr.md");
const PHENOTYPE_VOXEL_CATALOG: &str =
    include_str!("../../../../docs/requirements/phenotype-voxel-frnfr.md");
const AUTHVAULT_CATALOG: &str = include_str!("../../../../docs/requirements/authvault-frnfr.md");
const PHENOMCP_CATALOG: &str = include_str!("../../../../docs/requirements/phenomcp-frnfr.md");
const PHENOOBSERVABILITY_CATALOG: &str =
    include_str!("../../../../docs/requirements/phenoobservability-frnfr.md");

/// Arguments for the `seed-requirements` subcommand.
#[derive(Args, Debug)]
pub struct SeedRequirementsArgs {
    /// Path to the SQLite database file. Defaults to `./agileplus.db`.
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,

    /// Print a detailed per-story report.
    #[arg(long)]
    pub verbose: bool,
}

pub fn run(args: &SeedRequirementsArgs) -> anyhow::Result<()> {
    let conn = rusqlite::Connection::open(&args.db)?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    let runner = agileplus_sqlite::migrations::MigrationRunner::new(&conn);
    runner.run_all().map_err(|e| anyhow::anyhow!("{e}"))?;

    let initiatives = vec![
        Initiative {
            slug: "agileplus",
            title: "AgilePlus",
            catalog_markdown: AGILEPLUS_CATALOG,
        },
        Initiative {
            slug: "tracera",
            title: "Tracera",
            catalog_markdown: TRACERA_CATALOG,
        },
        Initiative {
            slug: "phenotype-voxel",
            title: "phenotype-voxel",
            catalog_markdown: PHENOTYPE_VOXEL_CATALOG,
        },
        Initiative {
            slug: "authvault",
            title: "Authvault",
            catalog_markdown: AUTHVAULT_CATALOG,
        },
        Initiative {
            slug: "phenomcp",
            title: "PhenoMCP",
            catalog_markdown: PHENOMCP_CATALOG,
        },
        Initiative {
            slug: "phenoobservability",
            title: "PhenoObservability",
            catalog_markdown: PHENOOBSERVABILITY_CATALOG,
        },
    ];

    let report =
        seed_requirements(&conn, &initiatives).map_err(|e| anyhow::anyhow!("seed failed: {e}"))?;

    println!(
        "Seeded {} epic(s) and {} story/stories across {} initiative(s).",
        report.epics_upserted,
        report.stories_upserted,
        report.initiatives.len()
    );

    if args.verbose {
        for init in &report.initiatives {
            println!(
                "\n  Epic [{}] id={}",
                init.epic_requirement_id, init.epic_id
            );
            for s in &init.stories {
                println!(
                    "    Story [{}] id={} status={:?}",
                    s.requirement_id, s.story_id, s.status
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{name}-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        path
    }

    #[test]
    fn seed_requirements_run_creates_database() {
        let db = temp_db_path("agileplus-seed-requirements");
        let args = SeedRequirementsArgs {
            db: db.clone(),
            verbose: false,
        };

        run(&args).unwrap();

        let conn = rusqlite::Connection::open(&db).unwrap();
        let epic_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM epics", [], |row| row.get(0))
            .unwrap();
        let story_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM stories", [], |row| row.get(0))
            .unwrap();

        assert!(epic_count >= 6);
        assert!(story_count > epic_count);

        let _ = std::fs::remove_file(db);
    }
}
