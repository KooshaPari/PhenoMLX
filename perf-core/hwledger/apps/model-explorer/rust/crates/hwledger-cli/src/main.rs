//! `hwledger-cli` — front-end binary for the hwledger model-explorer search
//! stack.
//!
//! Subcommand layout:
//!
//! - `model search`            — BM25 hybrid search against a tantivy index
//! - `model detail`            — full metadata for a single model id
//! - `model quants`            — list quantizations available for a model id
//! - `model similar`           — run a "more like this" lookup
//! - `model for-use-case`      — filter by use-case facet (agentic, coding, …)
//! - `model-ask`               — NL question → top-K context (RAG v1 stub)
//! - `seed build`              — populate a fresh tantivy index from HF
//! - `seed expand`             — neighborhood expansion around a seed list
//!
//! Every subcommand accepts `--json` to print a structured payload instead of
//! a comfy-table for scripting / piping.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use comfy_table::{ContentArrangement, Table};
use hwledger_search_core::{Facets, ModelKind, Query};
use hwledger_search_index::{
    collapse_variants, run_hybrid, CollapseRule, IndexHit,
};
use hwledger_search_ingest::{
    build_seed_index, expand_neighborhood, ExpansionConfig, HuggingFaceAdapter,
    PopulateGate, SeedBuild, SeedReport,
};

mod format;
mod store;

use format::{print_json, print_table, OutputFormat};
use store::{open_or_create_store, seed_sink_for, write_store, SharedStore};

/// Top-level CLI.
#[derive(Debug, Parser)]
#[command(name = "hwledger-cli", version, about = "hwledger model-explorer CLI")]
struct Cli {
    /// Path to the tantivy index directory.
    #[arg(long, env = "HWLEDGER_INDEX", global = true, default_value = "./hwledger-index")]
    index: PathBuf,

    /// Emit machine-readable output (alias for `--format json`).
    ///
    /// Kept as a separate flag for backwards compatibility with v0.1
    /// scripts. New callers should prefer `--format <human|json>`.
    #[arg(long, global = true)]
    json: bool,

    /// Explicit output format. Wins over `--json` when both are passed.
    #[arg(long, global = true, value_enum)]
    format: Option<format::OutputFormat>,

    #[command(subcommand)]
    command: Command,
}

/// All top-level subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Search, inspect, and facet models.
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// Ask a natural-language question against the indexed corpus.
    ModelAsk {
        /// Free-text question.
        question: String,
        /// Cap on returned rows.
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Build or expand the seed index.
    Seed {
        #[command(subcommand)]
        action: SeedAction,
    },
}

/// `model …` sub-actions.
#[derive(Debug, Subcommand)]
enum ModelAction {
    /// Free-text BM25 hybrid search.
    Search(SearchArgs),
    /// Print the full metadata for one model id.
    Detail {
        /// `source::id` or bare `id` (the CLI assumes `hf::` when omitted).
        id: String,
    },
    /// List the quantization tags available for one model id.
    Quants {
        /// `source::id` or bare `id`.
        id: String,
    },
    /// Run a "more like this" lookup seeded by a model id.
    Similar {
        /// Source model id.
        id: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Return models scored for a particular use case.
    ForUseCase {
        /// Which use case to score against.
        #[arg(long, value_enum)]
        use_case: UseCase,
        /// Optional free-text query to combine with the use-case filter.
        #[arg(long, default_value = "")]
        text: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

/// Common args for the `model search` subcommand.
#[derive(Debug, Args)]
struct SearchArgs {
    /// Free-text query.
    text: String,
    /// Cap on returned rows.
    #[arg(long, default_value_t = 25)]
    limit: usize,
    /// Restrict to one or more model kinds (comma-separated, e.g.
    /// `instruct,coding`).
    #[arg(long, value_delimiter = ',')]
    kind: Vec<String>,
}

/// Use cases `model for-use-case` understands today.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum UseCase {
    /// Tool-using / agentic workloads.
    Agentic,
    /// Programming assistants.
    Coding,
    /// General reasoning / chain-of-thought.
    Reasoning,
    /// Embedding lookup.
    Embedding,
}

impl UseCase {
    fn as_str(self) -> &'static str {
        match self {
            UseCase::Agentic => "agentic",
            UseCase::Coding => "coding",
            UseCase::Reasoning => "reasoning",
            UseCase::Embedding => "embedding",
        }
    }
}

/// `seed …` sub-actions.
#[derive(Debug, Subcommand)]
enum SeedAction {
    /// Build a fresh seed index by fanning out HF queries.
    Build {
        /// Comma-separated HF search queries to seed the index with.
        #[arg(long, value_delimiter = ',')]
        queries: Vec<String>,
        /// Soft cap on total models to ingest.
        #[arg(long, default_value_t = 2000)]
        size: usize,
        /// If set, do NOT wipe the existing index before rebuilding.
        #[arg(long)]
        append: bool,
    },
    /// Expand a list of seed ids through the neighborhood crawl (v1 stub).
    Expand {
        /// Comma-separated seed ids.
        #[arg(long, value_delimiter = ',')]
        seeds: Vec<String>,
    },
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let json = OutputFormat::resolve(cli.format.as_ref(), cli.json);

    match cli.command {
        Command::Model { action } => handle_model(action, &cli.index, json),
        Command::ModelAsk { question, limit } => {
            cmd_model_ask(&question, limit, &cli.index, json)
        }
        Command::Seed { action } => handle_seed(action, &cli.index, json),
    }
}

/// Initialize a `tracing` subscriber; swallows the "already set" error so
/// tests that instantiate the binary multiple times don't panic. Logs are
/// routed to **stderr** so that `--json` consumers see only the JSON
/// envelope on stdout.
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

fn handle_model(action: ModelAction, index_dir: &Path, json: bool) -> Result<()> {
    match action {
        ModelAction::Search(args) => cmd_model_search(args, index_dir, json),
        ModelAction::Detail { id } => cmd_model_detail(&id, index_dir, json),
        ModelAction::Quants { id } => cmd_model_quants(&id, index_dir, json),
        ModelAction::Similar { id, limit } => cmd_model_similar(&id, limit, index_dir, json),
        ModelAction::ForUseCase { use_case, text, limit } => {
            cmd_model_for_use_case(use_case, &text, limit, index_dir, json)
        }
    }
}

// ---------------------------------------------------------------------------
// `model search`
// ---------------------------------------------------------------------------

fn cmd_model_search(args: SearchArgs, index_dir: &Path, json: bool) -> Result<()> {
    let store = open_or_create_store(index_dir)?;
    let mut facets = Facets::default();
    for raw in &args.kind {
        let kind = parse_kind(raw)?;
        if !facets.kinds.contains(&kind) {
            facets.kinds.push(kind);
        }
    }
    let q = Query {
        text: args.text.clone(),
        facets,
        sort: None,
        limit: args.limit,
    };
    let hits = run_hybrid_blocking(&store, &q)?;
    if json {
        print_json(&serde_json::json!({
            "query": args.text,
            "limit": args.limit,
            "results": hits,
        }))?;
    } else {
        print_search_table(&hits);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `model detail`
// ---------------------------------------------------------------------------

fn cmd_model_detail(id: &str, index_dir: &Path, json: bool) -> Result<()> {
    let store = open_or_create_store(index_dir)?;
    let canonical = canonicalize_id(id);
    let hits = store.search(&canonical, 1).context("tantivy search failed")?;
    let kind = store.kind_for_id(&canonical);
    let quants = store.quants_for_id(&canonical);

    let payload = serde_json::json!({
        "id": canonical,
        "found": !hits.is_empty(),
        "score": hits.first().map(|h| h.score),
        "kind": kind,
        "quants": quants,
    });

    if json {
        print_json(&payload)?;
    } else {
        let mut t = Table::new();
        t.set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["field", "value"]);
        t.add_row(vec![
            comfy_table::Cell::new("id"),
            comfy_table::Cell::new(&canonical),
        ]);
        t.add_row(vec![
            comfy_table::Cell::new("found"),
            comfy_table::Cell::new(if hits.is_empty() { "false" } else { "true" }),
        ]);
        if let Some(score) = hits.first().map(|h| h.score) {
            t.add_row(vec![
                comfy_table::Cell::new("score"),
                comfy_table::Cell::new(format!("{score:.4}")),
            ]);
        }
        if let Some(kind) = kind {
            t.add_row(vec![
                comfy_table::Cell::new("kind"),
                comfy_table::Cell::new(kind),
            ]);
        }
        if let Some(quants) = quants {
            t.add_row(vec![
                comfy_table::Cell::new("quants"),
                comfy_table::Cell::new(quants.join(", ")),
            ]);
        }
        print_table(&t);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `model quants`
// ---------------------------------------------------------------------------

fn cmd_model_quants(id: &str, index_dir: &Path, json: bool) -> Result<()> {
    let store = open_or_create_store(index_dir)?;
    let canonical = canonicalize_id(id);
    let quants = store.quants_for_id(&canonical).unwrap_or_default();

    if json {
        print_json(&serde_json::json!({
            "id": canonical,
            "quants": quants,
        }))?;
    } else if quants.is_empty() {
        println!("(no quantization tags known for {canonical})");
    } else {
        let mut t = Table::new();
        t.set_header(vec!["quant"]).set_content_arrangement(ContentArrangement::Dynamic);
        for q in &quants {
            t.add_row(vec![comfy_table::Cell::new(q)]);
        }
        print_table(&t);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `model similar`
// ---------------------------------------------------------------------------

fn cmd_model_similar(
    id: &str,
    limit: usize,
    index_dir: &Path,
    json: bool,
) -> Result<()> {
    let store = open_or_create_store(index_dir)?;
    let canonical = canonicalize_id(id);

    // v1 "more like this": re-issue a BM25 query using the model's id tokens
    // (without the source prefix). The post-filter (run_hybrid) keeps the
    // kind facet resolution cheap.
    let q = Query {
        text: strip_source_prefix(&canonical).to_string(),
        facets: Facets::default(),
        sort: None,
        limit: limit.max(1),
    };
    let hits = run_hybrid_blocking(&store, &q)?;

    // Drop the seed itself from the result if it appears.
    let hits: Vec<_> = hits.into_iter().filter(|r| r.id != canonical).collect();

    if json {
        print_json(&serde_json::json!({
            "seed": canonical,
            "limit": limit,
            "results": hits,
        }))?;
    } else {
        print_search_table(&hits);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `model for-use-case`
// ---------------------------------------------------------------------------

fn cmd_model_for_use_case(
    use_case: UseCase,
    text: &str,
    limit: usize,
    index_dir: &Path,
    json: bool,
) -> Result<()> {
    let store = open_or_create_store(index_dir)?;

    // v1 use-case filter: short-circuit by setting a single `kind` facet and
    // re-using run_hybrid. The rich `agentic_fit`/`coding_fit` numerics from
    // `hwledger-search-evals` will land in a later phase.
    let kind = match use_case {
        UseCase::Agentic => ModelKind::Agentic,
        UseCase::Coding => ModelKind::Coding,
        UseCase::Reasoning => ModelKind::Reasoning,
        UseCase::Embedding => ModelKind::Embedding,
    };
    let mut facets = Facets::default();
    facets.kinds.push(kind);

    let effective_text = if text.is_empty() {
        use_case.as_str().to_string()
    } else {
        text.to_string()
    };

    let q = Query {
        text: effective_text,
        facets,
        sort: Some("agentic_fit".to_string()),
        limit,
    };
    let hits = run_hybrid_blocking(&store, &q)?;

    if json {
        print_json(&serde_json::json!({
            "use_case": use_case.as_str(),
            "text": text,
            "limit": limit,
            "results": hits,
        }))?;
    } else {
        print_search_table(&hits);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `model-ask`
// ---------------------------------------------------------------------------

fn cmd_model_ask(question: &str, limit: usize, index_dir: &Path, json: bool) -> Result<()> {
    let store = open_or_create_store(index_dir)?;
    let q = Query::text(question).with_limit(limit);
    let hits = run_hybrid_blocking(&store, &q)?;

    // v1 RAG stub: just echo the top-K BM25 hits back as "context". A real
    // pipeline would chunk the card text + run cosine retrieval here.
    let context: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| {
            serde_json::json!({
                "id": h.id,
                "score": h.score,
                "snippet": "",
            })
        })
        .collect();

    let payload = serde_json::json!({
        "question": question,
        "limit": limit,
        "answer": format!("(stub) top-{} BM25 hits for: {}", hits.len(), question),
        "context": context,
    });

    if json {
        print_json(&payload)?;
    } else {
        println!("{}", payload["answer"].as_str().unwrap_or(""));
        if !hits.is_empty() {
            print_search_table(&hits);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `seed …`
// ---------------------------------------------------------------------------

fn handle_seed(action: SeedAction, index_dir: &Path, json: bool) -> Result<()> {
    match action {
        SeedAction::Build { queries, size, append } => {
            cmd_seed_build(queries, size, append, index_dir, json)
        }
        SeedAction::Expand { seeds } => cmd_seed_expand(seeds, index_dir, json),
    }
}

fn cmd_seed_build(
    queries: Vec<String>,
    size: usize,
    append: bool,
    index_dir: &Path,
    json: bool,
) -> Result<()> {
    if !append && index_dir.exists() {
        std::fs::remove_dir_all(index_dir).with_context(|| {
            format!("failed to wipe existing index at {}", index_dir.display())
        })?;
    }

    let store = open_or_create_store(index_dir)?;
    let mut sink = seed_sink_for(&store);

    let build = if queries.is_empty() {
        SeedBuild {
            queries: SeedBuild::default().queries,
            size,
        }
    } else {
        SeedBuild { queries, size }
    };

    let adapter = HuggingFaceAdapter::from_env().context("failed to build HF adapter")?;
    let report: SeedReport = build_seed_index(&adapter, &mut sink, &build);
    // Commit through the underlying tantivy handle so the freshly-written
    // segments become visible to a subsequent search in the same process.
    store.commit().context("tantivy commit failed")?;
    write_store(store.as_ref(), &report);

    if json {
        print_json(&serde_json::json!({
            "models_indexed": report.models_indexed,
            "errors": report.errors,
            "queries_run": report.queries_run,
        }))?;
    } else {
        println!(
            "seed build done: indexed={} errors={} queries_run={}",
            report.models_indexed, report.errors, report.queries_run
        );
    }
    Ok(())
}

fn cmd_seed_expand(seeds: Vec<String>, index_dir: &Path, json: bool) -> Result<()> {
    // Treat both "no flag passed" and "blank entries from `,,` / `--seeds ""`"
    // as "no seeds". Clap's `value_delimiter` collapses `--seeds ""` to an
    // empty string element, which would otherwise silently no-op.
    let seeds: Vec<String> = seeds
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if seeds.is_empty() {
        anyhow::bail!("at least one seed id is required (pass via --seeds)");
    }
    // Open the index so operators see a clear error if the directory is
    // missing or unreadable; expansion itself does not need the store, but
    // validating the index path up front gives a better failure story than
    // a silent no-op.
    let _store = open_or_create_store(index_dir)?;
    let adapter = HuggingFaceAdapter::from_env().context("failed to build HF adapter")?;
    let gate = PopulateGate::new();
    let _cfg = ExpansionConfig::default();
    let expanded = expand_neighborhood(&adapter, &gate, seeds.clone());

    if json {
        print_json(&serde_json::json!({
            "seeds": seeds,
            "expanded": expanded,
        }))?;
    } else {
        println!("expanded {} seed id(s):", seeds.len());
        for id in &expanded {
            println!("  - {id}");
        }
        println!(
            "(collapse preview for the first result slice: {})",
            collapse_preview(&expanded)
        );
    }
    Ok(())
}

fn collapse_preview(ids: &[String]) -> String {
    let hits: Vec<IndexHit> = ids
        .iter()
        .map(|id| IndexHit::new(id.clone(), 0.0))
        .collect();
    let groups = collapse_variants(hits, &CollapseRule::default());
    format!(
        "{} groups ({} ids)",
        groups.len(),
        groups.iter().map(|g| g.variants.len()).sum::<usize>()
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `Query`, run the hybrid driver synchronously, and unwrap.
fn run_hybrid_blocking(
    store: &SharedStore,
    q: &Query,
) -> Result<Vec<hwledger_search_core::FusedResult>> {
    let store = store.clone();
    let q = q.clone();
    let limit = q.limit.max(1);
    // `run_hybrid` is `async`-shaped but the body is sync; we run it inline
    // on a fresh single-thread runtime so we never have to enter async at
    // the call site. The shared store is `Send + Sync` (Arc-wrapped), so
    // it's safe to move into the future.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    let hits = rt.block_on(async move { run_hybrid(&store, &q, limit).await })?;
    Ok(hits)
}

/// Parse a CLI `--kind` string into a `ModelKind`.
fn parse_kind(s: &str) -> Result<ModelKind> {
    match s.to_ascii_lowercase().as_str() {
        "base" => Ok(ModelKind::Base),
        "instruct" => Ok(ModelKind::Instruct),
        "chat" => Ok(ModelKind::Chat),
        "reasoning" => Ok(ModelKind::Reasoning),
        "coding" => Ok(ModelKind::Coding),
        "agentic" => Ok(ModelKind::Agentic),
        "embedding" => Ok(ModelKind::Embedding),
        "reranker" => Ok(ModelKind::Reranker),
        "vision_language" => Ok(ModelKind::VisionLanguage),
        "vision_encoder" => Ok(ModelKind::VisionEncoder),
        "audio" => Ok(ModelKind::Audio),
        "merge" => Ok(ModelKind::Merge),
        "finetune" => Ok(ModelKind::Finetune),
        "adapter" => Ok(ModelKind::Adapter),
        "quant" => Ok(ModelKind::Quant),
        other => Err(anyhow::anyhow!("unknown model kind: {other}")),
    }
}

/// Turn `org/name` into `hf::org/name` if no source prefix is present.
fn canonicalize_id(id: &str) -> String {
    if id.contains("::") {
        id.to_string()
    } else {
        format!("hf::{id}")
    }
}

/// Strip the leading `source::` portion of a key.
fn strip_source_prefix(id: &str) -> &str {
    match id.find("::") {
        Some(idx) => &id[idx + 2..],
        None => id,
    }
}

fn print_search_table(hits: &[hwledger_search_core::FusedResult]) {
    if hits.is_empty() {
        println!("(no results)");
        return;
    }
    let mut t = Table::new();
    t.set_header(vec!["#", "id", "score", "kinds"])
        .set_content_arrangement(ContentArrangement::Dynamic);
    for (i, h) in hits.iter().enumerate() {
        let kinds = h
            .facets
            .kinds
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(",");
        t.add_row(vec![
            comfy_table::Cell::new(i + 1),
            comfy_table::Cell::new(&h.id),
            comfy_table::Cell::new(format!("{:.4}", h.score)),
            comfy_table::Cell::new(if kinds.is_empty() { "—" } else { kinds.as_str() }),
        ]);
    }
    print_table(&t);
}