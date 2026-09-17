// SPDX-License-Identifier: MIT OR Apache-2.0
//! Standalone seed binary: apply migrations + seed 4 Epics + N Stories from
//! the embedded FR/NFR catalogs into the target SQLite database.
//!
//! Usage:  cargo run -p agileplus-sqlite --bin seed_db -- [path/to/agileplus.db]
//! Default db path: agileplus.db  (relative to CWD)

use std::path::PathBuf;

use agileplus_sqlite::{
    migrations::MigrationRunner,
    seed::{seed_requirements, Initiative},
};

const AGILEPLUS_CATALOG: &str = include_str!("../../../../docs/requirements/agileplus-frnfr.md");
const TRACERA_CATALOG: &str = include_str!("../../../../docs/requirements/tracera-frnfr.md");
const PHENOTYPE_VOXEL_CATALOG: &str =
    include_str!("../../../../docs/requirements/phenotype-voxel-frnfr.md");
const AUTHVAULT_CATALOG: &str = include_str!("../../../../docs/requirements/authvault-frnfr.md");

fn main() -> anyhow::Result<()> {
    let db_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("agileplus.db"));

    println!("Seeding database at: {}", db_path.display());

    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    let runner = MigrationRunner::new(&conn);
    runner
        .run_all()
        .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;
    println!("Migrations applied.");

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
    ];

    let report =
        seed_requirements(&conn, &initiatives).map_err(|e| anyhow::anyhow!("seed failed: {e}"))?;

    println!(
        "Done: {} epic(s), {} story/stories across {} initiative(s).",
        report.epics_upserted,
        report.stories_upserted,
        report.initiatives.len()
    );

    Ok(())
}
