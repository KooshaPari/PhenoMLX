// SPDX-License-Identifier: MIT OR Apache-2.0
//! Standalone seed-requirements binary for the agileplus-sqlite crate.
//!
//! Ingests all 6 FR/NFR catalogs (AgilePlus, Tracera, phenotype-voxel, Authvault,
//! PhenoMCP, PhenoObservability) into the target SQLite database.
//! Idempotent — safe to run multiple times (upsert by requirement_id).
//!
//! Usage:
//!   cargo run -p agileplus-sqlite --bin seed_requirements -- --db agileplus.db

use std::path::PathBuf;

const AGILEPLUS_CATALOG: &str = include_str!("../../../../docs/requirements/agileplus-frnfr.md");
const TRACERA_CATALOG: &str = include_str!("../../../../docs/requirements/tracera-frnfr.md");
const PHENOTYPE_VOXEL_CATALOG: &str =
    include_str!("../../../../docs/requirements/phenotype-voxel-frnfr.md");
const AUTHVAULT_CATALOG: &str = include_str!("../../../../docs/requirements/authvault-frnfr.md");
const PHENOMCP_CATALOG: &str = include_str!("../../../../docs/requirements/phenomcp-frnfr.md");
const PHENOOBSERVABILITY_CATALOG: &str =
    include_str!("../../../../docs/requirements/phenoobservability-frnfr.md");

fn main() -> anyhow::Result<()> {
    // Simple arg parse: --db <path>
    let args: Vec<String> = std::env::args().collect();
    let db_path: PathBuf = {
        let mut p = PathBuf::from("agileplus.db");
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--db" && i + 1 < args.len() {
                p = PathBuf::from(&args[i + 1]);
                break;
            }
            i += 1;
        }
        p
    };

    println!("Opening database: {}", db_path.display());
    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    let runner = agileplus_sqlite::migrations::MigrationRunner::new(&conn);
    runner.run_all().map_err(|e| anyhow::anyhow!("{e}"))?;

    use agileplus_sqlite::seed::{seed_requirements, Initiative};

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

    for init in &report.initiatives {
        let done_count = init
            .stories
            .iter()
            .filter(|s| matches!(s.status, agileplus_domain::domain::story::StoryStatus::Done))
            .count();
        let todo_count = init.stories.len() - done_count;
        println!(
            "  [{}] epic_id={} stories={} (Done={} Todo={})",
            init.initiative_slug,
            init.epic_id,
            init.stories.len(),
            done_count,
            todo_count,
        );
    }

    Ok(())
}
