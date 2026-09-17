//! agileplus-triage â€” rule-based triage engine for synced items.
//!
//! # Modules
//! - `engine`: hexagonal classify port â€” pure `classify(item, rules) -> TriageOutcome`
//! - `classifier`: free-text keyword classifier (legacy / internal use)
//! - `backlog`: in-memory backlog store
//! - `adapter`: high-level orchestration combining classifier + store
//! - `router`: governance file (CLAUDE.md / AGENTS.md) generator
//! - `dedup`: token-Jaccard, fuzzy ratio, simhash, n-gram, hybrid dedup scorer
//! - `claim`: resource claim primitives (repo/branch/worktree) with TTL + heartbeat,
//!   structured `ClaimReason`, in-memory `ClaimStore` + `ClaimStoreTrait`
//! - `claim_store_sqlite`: SQLite-backed `ClaimStoreTrait` impl (feature `sqlite`)
//! - `repo_introspect`: git / mangled / no-git repo classification
//! - `minhash`: Broder MinHash signatures with k-permutation FNV-1a
//! - `bloom`: feature `bloom` â€” bitvec-backed Bloom filter for membership tests
//! - `embeddings`: `EmbeddingBackend` trait + `LocalMockEmbeddings`; feature `oai` adds
//!   `OaiEmbeddings` (api.openai.com), feature `voyage` adds `VoyageEmbeddings`
//! - `hybrid_pipeline`: MinHash-LSH candidate generation + embedding cosine
//!   verification + Jaccard tiebreak (`HybridDedup::build / find / run_dedup`)
//! - `ast_tokenize`: regex-based AST-aware tokenization for Rust and Python
//!
//! Traceability: FR-AGP-017, FR-AGP-018 (triage dedup primitives),
//! audit recs #1-#5 from `AUDIT_BLOC_VS_2026_SOTA.md`, and audit recs
//! #6 (structured `ClaimReason`), #7 (`claim_transfer`), #8 (SQLite
//! store behind `sqlite` feature).

use std::str::FromStr;

use anyhow::Result;
use clap::Args;

pub mod adapter;
pub mod ast_tokenize;
pub mod backlog;
pub mod bloom;
pub mod claim;
#[cfg(feature = "sqlite")]
pub mod claim_store_sqlite;
pub mod claim_watcher;
pub mod classifier;
#[cfg(feature = "codebert")]
pub mod codebert;
pub mod dedup;
pub mod embeddings;
pub mod engine;
pub mod hybrid_pipeline;
pub mod lsh;
pub mod minhash;
pub mod repo_introspect;
pub mod router;
pub mod tree_sitter;

/// Arguments for the `agileplus triage` subcommand.
#[derive(Debug, Clone, Args)]
pub struct TriageArgs {
    /// Text to classify.
    pub input: Vec<String>,

    /// Override classification type (bug, feature, idea, task, docs).
    #[arg(long, value_name = "TYPE")]
    pub r#type: Option<String>,

    /// Dry run: classify but don't add to backlog.
    #[arg(long)]
    pub dry_run: bool,

    /// Output format (table, json, yaml).
    #[arg(long, value_name = "FMT", default_value = "table")]
    pub output: String,
}

/// Parsed triage request ready for classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTriageInput {
    pub input: String,
    pub override_intent: Option<classifier::Intent>,
}

pub fn parse_intent(value: &str) -> Result<classifier::Intent> {
    classifier::Intent::from_str(&value.to_lowercase()).map_err(|error| anyhow::anyhow!(error))
}

pub fn parse_triage_input(args: &TriageArgs) -> Result<ParsedTriageInput> {
    let input = args.input.join(" ").trim().to_string();
    if input.is_empty() {
        anyhow::bail!("No input text provided. Usage: agileplus triage <text>");
    }

    let override_intent = args.r#type.as_deref().map(parse_intent).transpose()?;

    Ok(ParsedTriageInput {
        input,
        override_intent,
    })
}

pub fn classify_ticket(
    classifier: &classifier::TriageClassifier,
    parsed: &ParsedTriageInput,
) -> classifier::TriageResult {
    match parsed.override_intent {
        Some(intent) => classifier.classify_with_override(&parsed.input, intent),
        None => classifier.classify(&parsed.input),
    }
}

pub async fn run(args: TriageArgs) -> Result<()> {
    let parsed = parse_triage_input(&args)?;
    let classifier = classifier::TriageClassifier::new();
    let result = classify_ticket(&classifier, &parsed);

    if args.output == "json" {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Intent:      {}", result.intent);
        println!("Confidence:  {:.0}%", result.confidence * 100.0);
        if !result.matched_keywords.is_empty() {
            println!("Keywords:    {}", result.matched_keywords.join(", "));
        }
    }

    if !args.dry_run {
        println!("\nAdded to backlog as {} item.", result.intent);
    } else {
        println!("\n(dry run â€” not added to backlog)");
    }

    Ok(())
}

// Re-export the main engine surface so consumers can do:
//   use agileplus_triage::{SyncedItem, TriageRules, TriageOutcome, classify};
pub use engine::{classify, SyncedItem, TriageOutcome, TriageRule, TriageRules};

// Re-export BacklogStoreOps trait (used by adapter consumers)
pub use adapter::BacklogStoreOps;
pub use classifier::{Intent, TriageClassifier, TriageResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_triage_input_extracts_text_and_override() {
        let args = TriageArgs {
            input: vec!["Crash".into(), "on".into(), "login".into()],
            r#type: Some("bug".into()),
            dry_run: true,
            output: "table".into(),
        };

        let parsed = parse_triage_input(&args).unwrap();
        assert_eq!(parsed.input, "Crash on login");
        assert_eq!(parsed.override_intent, Some(classifier::Intent::Bug));
    }

    #[test]
    fn classify_ticket_uses_classifier_when_no_override() {
        let args = TriageArgs {
            input: vec!["Add".into(), "project".into(), "timeline".into()],
            r#type: None,
            dry_run: true,
            output: "json".into(),
        };

        let parsed = parse_triage_input(&args).unwrap();
        let result = classify_ticket(&classifier::TriageClassifier::new(), &parsed);
        assert_eq!(result.intent, classifier::Intent::Feature);
    }
}
