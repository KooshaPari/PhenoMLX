//! Output helpers: pretty tables for humans, JSON for scripts.
//!
//! The [`OutputFormat`] enum is the single source of truth for "should I
//! emit JSON or human output?" decisions. The CLI exposes it two ways:
//!
//! 1. A `--format <human|json>` flag for explicit selection.
//! 2. A legacy `--json` boolean kept for backwards compatibility with
//!    scripts written against v0.1.
//!
//! [`OutputFormat::resolve`] is the canonical precedence rule:
//! explicit `--format` wins, otherwise `--json` falls back to `Json`,
//! otherwise `Human`.

use std::io::Write;

use anyhow::Result;
use clap::ValueEnum;
use comfy_table::Table;
use serde::Serialize;

/// How a subcommand should render its results.
///
/// Maps onto a `clap::ValueEnum` so the CLI can accept `--format json`
/// (etc.) directly. The legacy `--json` boolean is folded into the same
/// decision via [`OutputFormat::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Pretty tables and prose for an interactive terminal.
    Human,
    /// A `serde_json` envelope on stdout for piping into other tools.
    Json,
}

impl OutputFormat {
    /// Resolve the effective output format.
    ///
    /// Precedence: an explicit `format` always wins; if the caller did
    /// not pass `--format`, fall back to the legacy `json` boolean.
    pub fn resolve(format: Option<&Self>, json: bool) -> bool {
        match format {
            Some(Self::Json) => true,
            Some(Self::Human) => false,
            None => json,
        }
    }
}

/// Render a `serde::Serialize` value as pretty JSON on stdout.
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let s = serde_json::to_string_pretty(value)?;
    println!("{s}");
    Ok(())
}

/// Write a pre-built `comfy_table::Table` to stdout.
pub fn print_table(table: &Table) {
    // comfy-table doesn't write to a writer by default in 7.x — print via Display.
    println!("{table}");
}

/// Suppress unused-warning on `Write` when feature-gating tests later.
#[allow(dead_code)]
fn _write_marker(_w: &mut dyn Write) {}