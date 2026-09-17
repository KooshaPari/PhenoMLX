// SPDX-License-Identifier: MIT OR Apache-2.0
//! Requirements seed — parse FR/NFR catalogs and upsert Epics + Stories.
//!
//! Traceability: FR-AGP-001, FR-TRC-016 (AgilePlus ↔ Tracera wiring)
//!
//! Each initiative maps to one Epic (upserted by `requirement_id = "EPIC-<slug>"`).
//! Each FR entry maps to one Story (upserted by `requirement_id = <fr-id>`).
//! Status mapping: "SHIPPED" → Done, everything else → Todo.
//! The seed is idempotent: re-running produces no duplicates.

pub mod catalog;
pub mod runner;

pub use runner::{seed_requirements, Initiative, SeedReport};
