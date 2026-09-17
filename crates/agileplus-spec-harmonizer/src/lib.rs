//! agileplus-spec-harmonizer
//!
//! Ingests four agent-spec formats (GSD, OpenSpec, BMAD-Method, Spec-Kitty) and
//! normalizes them into a single [`WorkPackage`] shape. The harmonizer is the
//! "ingest" half of the SDD/BDD/TDD/EDD traceability chain; the other half is
//! the [`tracera-core`] link store, which agileplus-subcmds bridges to via
//! the `agileplus tracera list` subcommand.
//!
//! Supported input formats:
//! - **GSD**: `## Task N: <title>` sections, optional `- [ ]` acceptance checkboxes
//! - **OpenSpec**: `## Spec <id>` sections with `## Acceptance Criteria` blocks
//! - **BMAD-Method**: `## Story <id>: <title>` sections with `## Acceptance Criteria`
//! - **Spec-Kitty**: `## Spec <id>` sections with `## Acceptance` blocks
//!
//! All four produce [`WorkPackage`]s that share the same `acceptance: Vec<AcceptanceCriterion>`
//! field, so downstream tooling (Tracera links, AgilePlus stories, dagctl pick/claim/done)
//! can operate on a single shape regardless of source.

pub mod parsers;

pub mod normalize;
pub mod emit;

/// A normalized work package produced by any of the four parsers.
///
/// The `source_format` field is preserved (not discarded) so a downstream
/// `agileplus harmonize --emit json` can round-trip back to the original.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkPackage {
    /// Stable id: `<format>-<seq>` for the parsed form, or caller-supplied.
    pub id: String,
    /// Short title, parsed from the `## Task N: <title>` or `## Spec <id>` line.
    pub title: String,
    /// Long-form description (everything between the heading and the first
    /// `## Acceptance`/`## Acceptance Criteria` subheading, or the next `## `).
    pub description: String,
    /// Acceptance criteria. Empty if the source had none.
    pub acceptance: Vec<AcceptanceCriterion>,
    /// "gsd" | "openspec" | "bmad" | "kitty" — preserved for round-trip.
    pub source_format: String,
    /// Original section anchor (e.g. "task-1", "spec-fr-12") — preserved.
    pub source_anchor: String,
}

/// A single acceptance criterion, parsed from `- [ ]` checkboxes or bullet points
/// under an `## Acceptance`/`## Acceptance Criteria` block.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AcceptanceCriterion {
    pub text: String,
    /// `true` for `- [x]` / `[x]`, `false` for `- [ ]` / `[ ]` / bullet.
    pub done: bool,
}

/// Re-export the parsers and the normalizer.
pub use parsers::{bmad, gsd, kitty, openspec, parse_for, Format};
pub use normalize::{merge, slug, stable_hash};
pub use emit::{emit_markdown, emit_ndjson};

/// Convenience: parse `text` as `format` and return a [`WorkPackage`] list.
pub fn parse(text: &str, format: Format) -> Result<Vec<WorkPackage>, String> {
    parse_for(text, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GSD_SAMPLE: &str = r#"
# Plan: build the harmonizer

## Task 1: Set up the Cargo workspace
Create a new crate `agileplus-spec-harmonizer` with a `parsers` module.

- [ ] Add a `Cargo.toml` with the `regex` dep
- [ ] Write the lib.rs module exports
- [x] Add a smoke test that asserts the lib compiles

## Task 2: Implement the OpenSpec parser
Parse `## Spec <id>` sections.
"#;

    #[test]
    fn parses_gsd_into_two_work_packages() {
        let pkgs = parse(GSD_SAMPLE, Format::Gsd).expect("gsd parse");
        assert_eq!(pkgs.len(), 2, "expected 2 GSD tasks, got {}", pkgs.len());
        assert_eq!(pkgs[0].title, "Set up the Cargo workspace");
        assert_eq!(pkgs[0].source_format, "gsd");
        assert_eq!(pkgs[0].acceptance.len(), 3, "expected 3 acceptance items");
        assert_eq!(pkgs[0].acceptance[2].done, true, "third acceptance should be checked");
    }
}
