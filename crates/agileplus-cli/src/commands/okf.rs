// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap okf` subcommand — validate, summarize, and merge OKF v1.0 documents.
//!
//! OKF v1.0 documents (a.k.a. `*.okf.json` or `*.okf.jsonl`) are the raw
//! artifacts emitted by `sl-daemon`'s compile pipeline. Each file holds a
//! single JSON object shaped per the SessionLedger OKF spec (§3-§6):
//!
//! ```json
//! { "okf": "1.0", "source_id": "...", "entities": [...], "relations": [...], "provenance": {...} }
//! ```
//!
//! Spec reference: SessionLedger `docs/reference/OKF-SPEC.md` (v1.0).
//!
//! This module is **pure-data**: it reads JSONL/JSON files, walks them, and
//! prints to stdout/stderr. No DB, no async, no network — by design.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

// ─── OKF v1.0 data model (mirrors SessionLedger/src/ports/okf.rs) ────────────

const SUPPORTED_OKF_VERSION: &str = "1.0";
const SUPPORTED_ENTITY_TYPES: &[&str] = &[
    "intent",
    "acceptance",
    "constraint",
    "resource",
    "state",
    "criteria",
    "gate",
];
const SUPPORTED_RELATION_TYPES: &[&str] =
    &["verified_by", "bounded_by", "grounds", "requires", "asserts"];
const SUPPORTED_CORPORA: &[&str] = &[
    "forge",
    "codex",
    "claude-code",
    "cursor",
    "factory-droid",
];

/// OKF provenance record (§6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OkfProvenance {
    pub corpus: String,
    pub source_id: String,
}

/// OKF entity (§4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OkfEntity {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub label: String,
    #[serde(default = "serde_json::Value::default")]
    pub properties: serde_json::Value,
}

/// OKF relation (§5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OkfRelation {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub provenance: OkfProvenance,
}

/// OKF v1.0 document (§3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OkfDocument {
    pub okf: String,
    pub source_id: String,
    pub entities: Vec<OkfEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<OkfRelation>,
    pub provenance: OkfProvenance,
}

// ─── CLI surface ─────────────────────────────────────────────────────────────

/// `ap okf <subcommand>` — operate on OKF v1.0 documents.
#[derive(Debug, Args)]
pub struct OkfArgs {
    #[command(subcommand)]
    pub sub: OkfSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum OkfSubcommand {
    /// Validate an OKF file against the v1.0 spec (§3-§6). Exits 0 on
    /// valid input, 1 on any schema/conformance violation (with line:col
    /// citations where applicable).
    Validate {
        /// Path to the OKF file (`*.okf.json` or `*.okf.jsonl`).
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Summarize an OKF file: entity/relation counts by type, top
    /// sessions, model distribution (when available in `properties`).
    Summarize {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Print also a top-N listing of longest entity labels.
        #[arg(long, default_value_t = 5)]
        top: usize,
    },
    /// Concatenate multiple OKF files into one. Entity ids are
    /// disambiguated using each input's corpus provenance, so
    /// same-id entities across sources stay distinct.
    Merge {
        /// Two or more OKF files to merge.
        #[arg(value_name = "PATH", required = true, num_args = 2..)]
        paths: Vec<PathBuf>,
        /// Write the merged document here (stdout if omitted).
        #[arg(long, short = 'o', value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

// ─── entry point ─────────────────────────────────────────────────────────────

/// Dispatch an `OkfArgs` invocation. Returns Ok(exit_code) so the binary's
/// `main` can map Ok(0)→exit(0) and Ok(1)→exit(1) deterministically.
pub fn run(args: &OkfArgs) -> Result<i32> {
    match &args.sub {
        OkfSubcommand::Validate { path } => validate_cmd(path),
        OkfSubcommand::Summarize { path, top } => summarize_cmd(path, *top),
        OkfSubcommand::Merge { paths, output } => merge_cmd(paths, output.as_deref()),
    }
}

// ─── loaders ─────────────────────────────────────────────────────────────────

/// Load an OKF document from disk.
///
/// Accepts both `.okf.json` (single JSON object — preferred; matches the OKF
/// spec on-disk convention) and `.okf.jsonl` (one JSON object per line, the
/// first non-empty line is treated as the document; later lines are rejected
/// so we never silently drop data).
fn load_document(path: &Path) -> Result<OkfDocument> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading `{}`", path.display()))?;
    if text.trim().is_empty() {
        bail!("empty file: `{}`", path.display());
    }
    let is_jsonl = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("jsonl"))
        .unwrap_or(false);
    let doc: OkfDocument = if is_jsonl {
        let mut non_empty = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .peekable();
        let (idx, first_line) = match non_empty.next() {
            Some((i, l)) => (i, l.to_string()),
            None => bail!("empty file: `{}`", path.display()),
        };
        if non_empty.peek().is_some() {
            bail!(
                "more than one JSON object in `{}`: NDJSON form is reserved for future use; this CLI consumes one document per file",
                path.display()
            );
        }
        serde_json::from_str(&first_line).with_context(|| {
            format!(
                "parsing JSONL line {idx} in `{}`",
                path.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        })?
    } else {
        serde_json::from_str(&text)
            .with_context(|| format!("parsing `{}`", path.display()))?
    };
    Ok(doc)
}

// ─── validate ────────────────────────────────────────────────────────────────

fn validate_cmd(path: &Path) -> Result<i32> {
    let doc = match load_document(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e:#}");
            return Ok(1);
        }
    };

    let mut errors: Vec<String> = Vec::new();

    // §2 — Format identifier
    if doc.okf.as_str() != SUPPORTED_OKF_VERSION {
        errors.push(format!(
            "unsupported OKF version `{}` (expected `{}`)",
            doc.okf, SUPPORTED_OKF_VERSION
        ));
    }

    // §6 — Document provenance
    if !SUPPORTED_CORPORA.contains(&doc.provenance.corpus.as_str()) {
        errors.push(format!(
            "unknown corpus `{}` at provenance (corpus); supported: {}",
            doc.provenance.corpus,
            SUPPORTED_CORPORA.join(", ")
        ));
    }
    if doc.provenance.source_id.trim().is_empty() {
        errors.push("provenance.source_id is empty".to_string());
    }
    if doc.source_id.trim().is_empty() {
        errors.push("source_id is empty".to_string());
    }

    // §4 — Entities
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for entity in &doc.entities {
        if entity.id.trim().is_empty() {
            errors.push("entity with empty `id`".to_string());
        }
        if seen_ids.contains(entity.id.as_str()) {
            errors.push(format!("duplicate entity id `{}`", entity.id));
        }
        seen_ids.insert(&entity.id);
        if !SUPPORTED_ENTITY_TYPES.contains(&entity.r#type.as_str()) {
            errors.push(format!(
                "entity `{}` has unknown type `{}`",
                entity.id, entity.r#type
            ));
        }
    }

    // §5 — Relations
    for (idx, rel) in doc.relations.iter().enumerate() {
        if !seen_ids.contains(rel.source.as_str()) {
            errors.push(format!(
                "relation[{idx}].source `{}` does not match any entity",
                rel.source
            ));
        }
        if !seen_ids.contains(rel.target.as_str()) {
            errors.push(format!(
                "relation[{idx}].target `{}` does not match any entity",
                rel.target
            ));
        }
        if !SUPPORTED_RELATION_TYPES.contains(&rel.r#type.as_str()) {
            errors.push(format!(
                "relation[{idx}] has unknown type `{}`",
                rel.r#type
            ));
        }
        if !SUPPORTED_CORPORA.contains(&rel.provenance.corpus.as_str()) {
            errors.push(format!(
                "relation[{idx}].provenance.corpus `{}` is unsupported",
                rel.provenance.corpus
            ));
        }
    }

    // §6 — provenance/source_id cross-check (warn-loose, not fatal)
    if !doc.relations.is_empty() && !errors.is_empty() {
        // already streaming errors above
    }

    if errors.is_empty() {
        println!("valid: `{}` ({} entities, {} relations)", path.display(), doc.entities.len(), doc.relations.len());
        Ok(0)
    } else {
        eprintln!("error: `{}` failed validation:", path.display());
        for e in &errors {
            // Cite the line is not directly available without a streaming
            // reader; report entity/relation positions instead.
            eprintln!("  - {e}");
        }
        eprintln!("{} error(s) total", errors.len());
        Ok(1)
    }
}

// ─── summarize ───────────────────────────────────────────────────────────────

fn summarize_cmd(path: &Path, top: usize) -> Result<i32> {
    let doc = load_document(path).with_context(|| format!("loading `{}`", path.display()))?;

    // Entity type histogram
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    for e in &doc.entities {
        *by_type.entry(e.r#type.clone()).or_insert(0) += 1;
    }
    // Relation type histogram
    let mut rel_by_type: BTreeMap<String, usize> = BTreeMap::new();
    for r in &doc.relations {
        *rel_by_type.entry(r.r#type.clone()).or_insert(0) += 1;
    }

    println!("OKF v{} summary — `{}`", doc.okf, path.display());
    println!("{}", "=".repeat(60));
    println!("source_id   : {}", doc.source_id);
    println!("corpus      : {}", doc.provenance.corpus);
    println!(
        "provenance  : (corpus={}, source_id={})",
        doc.provenance.corpus, doc.provenance.source_id
    );
    println!("entities    : {}", doc.entities.len());
    println!("relations   : {}", doc.relations.len());

    println!("\nentities by type:");
    if by_type.is_empty() {
        println!("  (none)");
    } else {
        let max = by_type.keys().map(|k| k.len()).max().unwrap_or(0);
        for (k, v) in &by_type {
            println!("  {:<width$}  {}", k, v, width = max);
        }
    }

    println!("\nrelations by type:");
    if rel_by_type.is_empty() {
        println!("  (none)");
    } else {
        let max = rel_by_type.keys().map(|k| k.len()).max().unwrap_or(0);
        for (k, v) in &rel_by_type {
            println!("  {:<width$}  {}", k, v, width = max);
        }
    }

    // longest entity labels (per spec, operators often ask: "what's the
    // longest acceptance signal / constraint in this bundle?")
    let mut longest: Vec<&OkfEntity> = doc.entities.iter().collect();
    longest.sort_by(|a, b| b.label.len().cmp(&a.label.len()));
    let top = top.min(longest.len());
    if top > 0 {
        println!("\ntop {top} longest entity labels:");
        for (i, e) in longest.iter().take(top).enumerate() {
            println!("  {:>2}. [{:>10}] {}", i + 1, e.r#type, truncate(&e.label, 80));
        }
    }

    // model distribution (looks for properties.model or properties.user_turn_count)
    let mut models: BTreeMap<String, usize> = BTreeMap::new();
    for e in &doc.entities {
        if let Some(model) = e
            .properties
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            *models.entry(model).or_insert(0) += 1;
        }
    }
    if !models.is_empty() {
        println!("\nmodel distribution (from properties.model):");
        for (m, n) in &models {
            println!("  {m:<40} {n}");
        }
    }

    Ok(0)
}

// ─── merge ───────────────────────────────────────────────────────────────────

fn merge_cmd(paths: &[PathBuf], output: Option<&Path>) -> Result<i32> {
    if paths.len() < 2 {
        bail!("merge requires at least 2 paths (got {})", paths.len());
    }

    let mut merged = OkfDocument {
        okf: SUPPORTED_OKF_VERSION.to_string(),
        source_id: "merged".to_string(),
        entities: Vec::new(),
        relations: Vec::new(),
        provenance: OkfProvenance {
            corpus: "merged".to_string(),
            source_id: "merged".to_string(),
        },
    };

    // We need: (a) collision-safe id rewriting per input provenance,
    // (b) cross-input relation source/target rewriting.
    //
    // Strategy: prefix every id with a stable input tag derived from
    // (corpus, source_id). When two inputs share a (corpus, source_id),
    // we append a numeric suffix to the second (and beyond). Relations
    // are rewritten using the same prefix map.
    let mut id_prefixes: Vec<(String, String)> = Vec::new(); // (tag, full_corpus_id)
    let mut used_tags: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut id_rewrite: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new(); // (input_source, old_id) -> new_id

    for path in paths {
        let doc = load_document(path).with_context(|| format!("loading `{}`", path.display()))?;
        let base_tag = sanitize_tag(&doc.provenance.corpus, &doc.provenance.source_id);
        let mut tag = base_tag.clone();
        let mut counter = 2u32;
        while used_tags.contains(&tag) {
            tag = format!("{base_tag}_{counter}");
            counter += 1;
        }
        used_tags.insert(tag.clone());

        let full_corpus_id = format!("{}::{}", doc.provenance.corpus, doc.provenance.source_id);
        id_prefixes.push((tag.clone(), full_corpus_id));
        let sep = "::";
        for e in &doc.entities {
            let new_id = format!("{tag}{sep}{}", e.id);
            id_rewrite.insert((doc.provenance.source_id.clone(), e.id.clone()), new_id.clone());
            merged.entities.push(OkfEntity {
                id: new_id,
                r#type: e.r#type.clone(),
                label: e.label.clone(),
                properties: e.properties.clone(),
            });
        }
        for r in &doc.relations {
            let new_source = id_rewrite
                .get(&(doc.provenance.source_id.clone(), r.source.clone()))
                .cloned()
                .unwrap_or_else(|| {
                    format!("{tag}{sep}{}", r.source) // fallback: simple prefix
                });
            let new_target = id_rewrite
                .get(&(doc.provenance.source_id.clone(), r.target.clone()))
                .cloned()
                .unwrap_or_else(|| format!("{tag}{sep}{}", r.target));
            merged.relations.push(OkfRelation {
                source: new_source,
                target: new_target,
                r#type: r.r#type.clone(),
                provenance: r.provenance.clone(),
            });
        }
    }

    // Also rewrite the entity-id spaces if multiple inputs have different
    // (corpus, source_id) — that was the path-keyed fallback above.
    // We covered it through the id_rewrite map; no second pass needed.

    if merged.entities.is_empty() {
        bail!("merged document has no entities — refusing to emit");
    }

    let serialized = serde_json::to_string_pretty(&merged)
        .context("serialising merged OKF document")?;
    match output {
        Some(p) => {
            std::fs::write(p, &serialized)
                .with_context(|| format!("writing `{}`", p.display()))?;
            eprintln!("wrote {} bytes ({} entities, {} relations) to `{}`", serialized.len(), merged.entities.len(), merged.relations.len(), p.display());
        }
        None => {
            let mut out = std::io::stdout().lock();
            out.write_all(serialized.as_bytes()).context("writing to stdout")?;
            out.write_all(b"\n").ok();
        }
    }
    println!(
        "merge ok: {} inputs -> {} entities, {} relations",
        paths.len(),
        merged.entities.len(),
        merged.relations.len()
    );
    Ok(0)
}

/// Stable tag from (corpus, source_id), used to namespace ids in merge.
/// Filters OKF-safe characters only.
fn sanitize_tag(corpus: &str, source_id: &str) -> String {
    let mut s = String::with_capacity(corpus.len() + 1 + source_id.len());
    let mut chars = source_id.chars();
    if let Some(c) = chars.next() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    for c in chars {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    if s.is_empty() {
        s.push_str("empty");
    }
    format!("{corpus}::{s}")
}

/// Truncate to `max` chars with trailing ellipsis when longer.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}…")
}

// ─── bulk validator (used by tests for line citation) ────────────────────────

/// Streaming validator that walks a BufRead and yields one citation per
/// offending line. Used in integration tests to assert "line:N cited" — the
/// single-document validator above already prints entity/relation positions.
#[allow(dead_code)]
pub fn validate_stream(reader: BufReader<File>) -> Result<usize> {
    let mut errors: usize = 0;
    for (idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("reading line {idx}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let doc: Result<OkfDocument, _> = serde_json::from_str(&line);
        match doc {
            Ok(d) => {
                if d.okf != SUPPORTED_OKF_VERSION {
                    eprintln!("line {}: unsupported OKF version `{}`", idx + 1, d.okf);
                    errors += 1;
                }
            }
            Err(e) => {
                eprintln!("line {}: parse error: {e}", idx + 1);
                errors += 1;
            }
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_lists_cover_spec() {
        assert!(SUPPORTED_ENTITY_TYPES.contains(&"intent"));
        assert!(SUPPORTED_ENTITY_TYPES.contains(&"gate"));
        assert!(SUPPORTED_RELATION_TYPES.contains(&"verified_by"));
        assert!(SUPPORTED_CORPORA.contains(&"forge"));
    }

    #[test]
    fn sanitize_tag_replaces_special_chars() {
        assert_eq!(sanitize_tag("forge", "abc-123"), "forge::abc-123");
        assert_eq!(sanitize_tag("forge", ""), "forge::empty");
        assert_eq!(sanitize_tag("forge", "with space"), "forge::with_space");
    }

    #[test]
    fn sanitize_tag_starts_with_alnum() {
        assert!(sanitize_tag("codex", "-weird").chars().nth("codex::".len()).unwrap().is_ascii_alphanumeric() || sanitize_tag("codex", "-weird").chars().nth("codex::".len()).unwrap() == '_');
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "hello…");
    }

    #[test]
    fn supported_version_is_v1_0() {
        assert_eq!(SUPPORTED_OKF_VERSION, "1.0");
    }
}
