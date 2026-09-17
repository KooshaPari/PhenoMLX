// SPDX-License-Identifier: MIT OR Apache-2.0
// NOTE: Modules marked with `// STUB` have unresolved dependencies (missing
// crate additions or missing domain types) and are temporarily excluded from
// compilation until those upstream gaps are filled.  They are kept in the
// source tree for reference.

pub mod cockpit;
pub mod cockpit_read;
pub mod dashboard;
pub mod fix_list;
pub mod list;
pub mod list_epics;
pub mod list_projects;
pub mod list_stories;
pub mod list_tests;
pub mod mvp;
pub mod okf;
pub mod repl;
pub mod rubric;
pub mod seed_requirements;
pub mod trace;
pub mod worklog;

// ── SDD core modules (wired into the CLI binary) ──────────────────────────────
pub mod branch; // OK: VcsPort only
pub mod cycle;
pub mod governance; // OK: VcsPort read_artifact only
pub mod module;
pub mod queue;
pub mod specify;

// ── full-deps SDD modules ─────────────────────────────────────────────────────
#[cfg(feature = "full-deps")]
pub mod implement;
#[cfg(feature = "full-deps")]
pub mod plan;
#[cfg(feature = "full-deps")]
pub mod pr_builder;
#[cfg(feature = "full-deps")]
pub mod research;
#[cfg(feature = "full-deps")]
pub mod retrospective;
#[cfg(feature = "full-deps")]
pub mod review_loop;
#[cfg(feature = "full-deps")]
pub mod scheduler;
#[cfg(feature = "full-deps")]
pub mod scope;
#[cfg(feature = "full-deps")]
pub mod ship;
#[cfg(feature = "full-deps")]
pub mod triage;
#[cfg(feature = "full-deps")]
pub mod validate;
