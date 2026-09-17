// SPDX-License-Identifier: MIT OR Apache-2.0
//! `agileplus dag` command implementation.
//!
//! Block-equivalent of `dagctl` (the greenfield sibling) backed by the
//! `agileplus-application` use-case layer that wraps `agileplus-triage`
//! (dedup, claim, repo_introspect) and `agileplus-graph` (topo sort,
//! parallel layers). State is held in a single in-memory `AppState` per
//! CLI invocation; a SQLite-backed port is wired in via
//! `agileplus-sqlite` in a follow-up workflow.
//!
//! Subcommands
//! â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
//!   pick         â€” list pickable work packages for an agent
//!   claim        â€” claim a resource (repo/branch/worktree/subproject)
//!   release      â€” release a claim by id
//!   heartbeat    â€” refresh a claim's last_heartbeat
//!   done         â€” mark a work package done and release its claim
//!   dedup        â€” find duplicate WP candidates above a threshold
//!   dedup-explain â€” explain a single pair's similarity breakdown
//!   scan         â€” inspect a directory tree and classify repos
//!   topology     â€” print topo order + parallel layers
//!   where        â€” context snapshot for the current working directory
//!
//! Traceability: FR-AGP-018 (dedup), FR-AGP-019 (claim),
//! FR-AGP-020 (repo_introspect), FR-AGP-021 (graph topology),
//! FR-AGP-022 (CLI dag subcommands).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};

use agileplus_application::dto::{
    ClaimKind, ClaimReason, ClaimRequest, DedupRequest, DoneRequest, HeartbeatRequest, PickRequest,
    ReleaseRequest, ScanRequest, TopologyRequest, WhereRequest,
};
use agileplus_application::use_cases::triage::{AppState, TopologyReport, WpRepository};
use agileplus_triage::dedup::{find_duplicates, hybrid_score, token_jaccard};

// â”€â”€ Singleton AppState â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
//
// We back the CLI with a static in-memory `AppState` so all subcommands
// share claims. A more durable wiring (sqlite-backed `WpRepository` +
// per-resource flock) is the next workflow.

use std::sync::OnceLock;

fn shared_state() -> &'static Mutex<AppState<InMemoryWpRepo>> {
    static STATE: OnceLock<Mutex<AppState<InMemoryWpRepo>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(AppState::new(InMemoryWpRepo::default())))
}

// â”€â”€ Subcommand tree â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Args)]
pub struct DagArgs {
    #[command(subcommand)]
    pub cmd: DagCmd,
}

#[derive(Debug, Subcommand)]
pub enum DagCmd {
    /// List pickable work packages for an agent.
    Pick(PickArgs),
    /// Claim a resource.
    Claim(ClaimArgs),
    /// Release a claim by id.
    Release(ReleaseArgs),
    /// Refresh a claim's heartbeat.
    Heartbeat(HeartbeatArgs),
    /// Mark a work package done and release its claim.
    Done(DoneArgs),
    /// Find duplicate WP candidates.
    Dedup(DedupArgs),
    /// Explain why two items are/aren't considered duplicates.
    DedupExplain(DedupExplainArgs),
    /// Inspect a directory tree and classify repos.
    Scan(ScanArgs),
    /// Print topology (topo order + parallel layers).
    Topology(TopologyArgs),
    /// Context snapshot for the current working directory.
    Where(WhereArgs),
    /// Add a work package to the in-memory store (no dedup check; for tests).
    Add(AddArgs),
}

// â”€â”€ Arg structs â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Args)]
pub struct PickArgs {
    /// Agent id.
    #[arg(long)]
    pub agent: String,
    /// Maximum number of items to return.
    #[arg(long, default_value_t = 5)]
    pub limit: usize,
    /// Filter by lane.
    #[arg(long)]
    pub lane: Option<String>,
    /// Filter by category.
    #[arg(long)]
    pub category: Option<String>,
}

#[derive(Debug, Args)]
pub struct ClaimArgs {
    /// Agent id.
    #[arg(long)]
    pub agent: String,
    /// Resource to claim (path or logical id).
    #[arg(long)]
    pub resource: String,
    /// Kind of claim: repo, branch, worktree, subproject.
    #[arg(long, value_name = "KIND", default_value = "subproject")]
    pub kind: String,
    /// TTL in seconds (default 3600).
    #[arg(long, default_value_t = 3600)]
    pub ttl: i64,
    /// Optional reason (e.g. "task:abc-123").
    #[arg(long)]
    pub reason: Option<String>,
    /// Optional explicit claim id; defaults to "<agent>:<resource>".
    #[arg(long)]
    pub claim_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct ReleaseArgs {
    /// Claim id to release.
    #[arg(long)]
    pub claim_id: String,
}

#[derive(Debug, Args)]
pub struct HeartbeatArgs {
    /// Claim id to refresh.
    #[arg(long)]
    pub claim_id: String,
}

#[derive(Debug, Args)]
pub struct DoneArgs {
    /// Agent id.
    #[arg(long)]
    pub agent: String,
    /// Work package id.
    #[arg(long)]
    pub task: String,
    /// Claim id to release.
    #[arg(long)]
    pub claim_id: String,
    /// Optional result message.
    #[arg(long)]
    pub result: Option<String>,
}

#[derive(Debug, Args)]
pub struct DedupArgs {
    /// File of "<id>\t<description>" lines, or "-" for stdin.
    #[arg(long, default_value = "-")]
    pub from: String,
    /// Hybrid threshold (0.0 - 1.0).
    #[arg(long, default_value_t = 0.75)]
    pub threshold: f64,
}

#[derive(Debug, Args)]
pub struct DedupExplainArgs {
    /// First text to compare.
    #[arg(long)]
    pub a: String,
    /// Second text to compare.
    #[arg(long)]
    pub b: String,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// One or more roots to inspect.
    #[arg(required = true)]
    pub roots: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TopologyArgs {
    /// Optional root work package id (unused in the in-memory port).
    #[arg(long)]
    pub root: Option<String>,
}

#[derive(Debug, Args)]
pub struct WhereArgs {
    /// Directory to inspect (defaults to current dir).
    #[arg(long)]
    pub cwd: Option<String>,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Work package id.
    pub wp_id: String,
    /// Title / description.
    #[arg(long)]
    pub title: String,
    /// Initial state.
    #[arg(long, default_value = "ready")]
    pub state: String,
    /// Comma-separated dependency ids.
    #[arg(long, default_value = "")]
    pub depends: String,
}

// â”€â”€ In-memory WpRepository â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Default)]
pub struct InMemoryWpRepo {
    items: HashMap<String, agileplus_application::dto::PickedItem>,
}

impl WpRepository for InMemoryWpRepo {
    fn list_pickable(
        &self,
        _agent: &str,
        lane: Option<&str>,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<agileplus_application::dto::PickedItem>> {
        let _ = (lane, category);
        Ok(self
            .items
            .values()
            .filter(|i| i.state == "ready")
            .take(limit)
            .cloned()
            .collect())
    }

    fn all_for_export(
        &self,
        _with_side: bool,
    ) -> Result<Vec<agileplus_application::dto::PickedItem>> {
        Ok(self.items.values().cloned().collect())
    }

    fn add_dependency(&mut self, from: &str, to: &str) -> Result<()> {
        if let Some(item) = self.items.get_mut(from) {
            if !item.dependencies.iter().any(|d| d == to) {
                item.dependencies.push(to.to_string());
            }
            Ok(())
        } else {
            Err(anyhow!("unknown wp_id: {from}"))
        }
    }

    fn mark_done(&mut self, wp_id: &str) -> Result<()> {
        if let Some(item) = self.items.get_mut(wp_id) {
            item.state = "done".to_string();
            Ok(())
        } else {
            Err(anyhow!("unknown wp_id: {wp_id}"))
        }
    }

    fn claim_count(&self) -> usize {
        0
    }
    fn wp_count(&self) -> usize {
        self.items.len()
    }
    fn stage_count(&self) -> usize {
        8
    }
}

// â”€â”€ Dispatcher â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub async fn run_dag(args: DagArgs) -> Result<()> {
    match args.cmd {
        DagCmd::Pick(a) => cmd_pick(a).await,
        DagCmd::Claim(a) => cmd_claim(a).await,
        DagCmd::Release(a) => cmd_release(a).await,
        DagCmd::Heartbeat(a) => cmd_heartbeat(a).await,
        DagCmd::Done(a) => cmd_done(a).await,
        DagCmd::Dedup(a) => cmd_dedup(a).await,
        DagCmd::DedupExplain(a) => cmd_dedup_explain(a).await,
        DagCmd::Scan(a) => cmd_scan(a).await,
        DagCmd::Topology(a) => cmd_topology(a).await,
        DagCmd::Where(a) => cmd_where(a).await,
        DagCmd::Add(a) => cmd_add(a).await,
    }
}

// â”€â”€ Subcommand impls â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

async fn cmd_pick(a: PickArgs) -> Result<()> {
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let req = PickRequest {
        agent_id: a.agent,
        limit: a.limit,
        lane: a.lane,
        category: a.category,
    };
    let items = state.pick(&req)?;
    if items.is_empty() {
        println!("(no pickable items)");
    } else {
        for it in items {
            println!("{}\t{}\t{}", it.wp_id, it.state, it.title);
        }
    }
    Ok(())
}

async fn cmd_claim(a: ClaimArgs) -> Result<()> {
    let kind = parse_claim_kind(&a.kind)?;
    let claim_id = a
        .claim_id
        .unwrap_or_else(|| format!("{}:{}", a.agent, a.resource));
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    // Accept a structured prefix on `--reason` to opt into one of the
    // typed variants: `task:<id>`, `branch:<name>`, `subproject:<name>`,
    // `wip:<id>`. Anything else (including the empty string) becomes
    // `ClaimReason::Manual(<value>)`. This keeps the CLI ergonomic for
    // humans while preserving the structured typing on the wire.
    let reason = parse_claim_reason(a.reason.as_deref());
    let req = ClaimRequest {
        claim_id,
        resource: a.resource,
        kind,
        agent_id: a.agent,
        ttl_seconds: a.ttl,
        reason,
    };
    let claim = state.claim(&req)?;
    println!(
        "claimed: id={} kind={:?} resource={} agent={} ttl={}s",
        claim.id, claim.kind, claim.resource, claim.agent_id, claim.ttl_seconds
    );
    Ok(())
}

async fn cmd_release(a: ReleaseArgs) -> Result<()> {
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let ok = state.release(&ReleaseRequest {
        claim_id: a.claim_id.clone(),
    })?;
    println!(
        "release({}): {}",
        a.claim_id,
        if ok { "ok" } else { "not found" }
    );
    Ok(())
}

async fn cmd_heartbeat(a: HeartbeatArgs) -> Result<()> {
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let ok = state.heartbeat(&HeartbeatRequest {
        claim_id: a.claim_id.clone(),
    })?;
    println!(
        "heartbeat({}): {}",
        a.claim_id,
        if ok { "ok" } else { "not found" }
    );
    Ok(())
}

async fn cmd_done(a: DoneArgs) -> Result<()> {
    let mut state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let req = DoneRequest {
        claim_id: a.claim_id,
        wp_id: a.task,
        result: a.result,
    };
    let ok = state.done(&req)?;
    println!("done: {}", if ok { "ok" } else { "failed" });
    Ok(())
}

async fn cmd_dedup(a: DedupArgs) -> Result<()> {
    let items = read_id_text(&a.from)?;
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let cands = state.dedup(&DedupRequest {
        items,
        threshold: a.threshold,
    })?;
    if cands.is_empty() {
        println!("(no duplicate groups @ threshold {})", a.threshold);
        return Ok(());
    }
    for c in cands {
        println!(
            "{:<24}  {:<24}  hybrid={:.3}  jaccard={:.3}  simhash_dist={}",
            c.a_id, c.b_id, c.hybrid_score, c.token_jaccard, c.simhash_distance
        );
    }
    Ok(())
}

async fn cmd_dedup_explain(a: DedupExplainArgs) -> Result<()> {
    let pair = vec![
        ("a".to_string(), a.a.clone()),
        ("b".to_string(), a.b.clone()),
    ];
    let cands = find_duplicates(&pair, 0.0);
    let j = token_jaccard(&a.a, &a.b);
    let (h, _fj, _nj, _lr, sd) = hybrid_score(&a.a, &a.b);
    println!("a = {:?}", a.a);
    println!("b = {:?}", a.b);
    println!("token_jaccard    = {j:.4}");
    println!("hybrid_score     = {h:.4}");
    println!("simhash_distance = {sd}");
    if let Some(c) = cands.first() {
        println!(
            "candidate_pair   = {} vs {} (hybrid {:.4})",
            c.a_id, c.b_id, c.hybrid_score
        );
    }
    Ok(())
}

async fn cmd_scan(a: ScanArgs) -> Result<()> {
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let infos = state.scan(&ScanRequest {
        roots: a.roots.clone(),
        max_depth: None,
    })?;
    if infos.is_empty() {
        println!("(no roots scanned)");
    }
    for info in infos {
        println!(
            "path={}\tstate={:?}\tbranch={:?}\tbranches={}\tworktrees={}\thygiene={}",
            info.path,
            info.state,
            info.current_branch,
            info.branches.len(),
            info.worktrees.len(),
            info.hygiene_score
        );
    }
    Ok(())
}

async fn cmd_topology(a: TopologyArgs) -> Result<()> {
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let _ = a.root;
    let report: TopologyReport = state.topology(&TopologyRequest { root_wp: a.root })?;
    println!("topo order: {} nodes", report.topo.order.len());
    for (i, layer) in report.layers.iter().enumerate() {
        println!("  layer {i} (width={}): {}", layer.len(), layer.join(", "));
    }
    if let Some(cycle) = report.topo.cycle {
        println!("CYCLE DETECTED: {}", cycle.join(" -> "));
    }
    Ok(())
}

async fn cmd_where(a: WhereArgs) -> Result<()> {
    let cwd = a
        .cwd
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let _resp = state.where_am_i(&WhereRequest {
        cwd: cwd.display().to_string(),
    })?;
    // The in-memory port doesn't yet implement where_am_i; fall back to a
    // direct scan if the response is empty.
    let infos = state.scan(&ScanRequest {
        roots: vec![cwd.display().to_string()],
        max_depth: None,
    })?;
    for info in infos {
        println!(
            "repo={}\tstate={:?}\tbranch={:?}\thygiene={}",
            info.path, info.state, info.current_branch, info.hygiene_score
        );
    }
    Ok(())
}

async fn cmd_add(a: AddArgs) -> Result<()> {
    let mut state = shared_state().lock().map_err(|e| anyhow!("{e}"))?;
    let deps: Vec<String> = a
        .depends
        .split(',')
        .filter_map(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .collect();
    state.wp_repo.items.insert(
        a.wp_id.clone(),
        agileplus_application::dto::PickedItem {
            wp_id: a.wp_id.clone(),
            title: a.title.clone(),
            state: a.state.clone(),
            dependencies: deps,
        },
    );
    println!(
        "added: {} ({} deps)",
        a.wp_id,
        state.wp_repo.items[&a.wp_id].dependencies.len()
    );
    Ok(())
}

// â”€â”€ Helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn parse_claim_kind(s: &str) -> Result<ClaimKind> {
    match s.to_ascii_lowercase().as_str() {
        "repo" => Ok(ClaimKind::Repo),
        "branch" => Ok(ClaimKind::Branch),
        "worktree" => Ok(ClaimKind::Worktree),
        "subproject" => Ok(ClaimKind::Subproject),
        other => Err(anyhow!("unknown claim kind: {other}")),
    }
}

/// Translate the human-typed `--reason` string into a [`ClaimReason`].
///
/// The CLI uses prefix syntax to opt into the structured variants:
/// `task:<id>` -> `TaskRef`, `branch:<name>` -> `Branch`,
/// `subproject:<name>` -> `Subproject`, `wip:<id>` -> `WipRun`. Anything
/// else (including the empty / `None` value) becomes
/// `ClaimReason::Manual(<value>)`.
fn parse_claim_reason(value: Option<&str>) -> ClaimReason {
    let s = match value {
        Some(v) => v,
        None => return ClaimReason::default(),
    };
    if let Some(rest) = s.strip_prefix("task:") {
        ClaimReason::TaskRef(rest.to_string())
    } else if let Some(rest) = s.strip_prefix("branch:") {
        ClaimReason::Branch(rest.to_string())
    } else if let Some(rest) = s.strip_prefix("subproject:") {
        ClaimReason::Subproject(rest.to_string())
    } else if let Some(rest) = s.strip_prefix("wip:") {
        ClaimReason::WipRun(rest.to_string())
    } else {
        ClaimReason::Manual(s.to_string())
    }
}

fn read_id_text(from: &str) -> Result<Vec<(String, String)>> {
    use std::io::{self, BufRead};
    let r: Box<dyn BufRead> = if from == "-" {
        Box::new(io::stdin().lock())
    } else {
        Box::new(std::fs::File::open(from).map(io::BufReader::new)?)
    };
    let mut out = vec![];
    for line in r.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((id, text)) = line.split_once('\t') {
            out.push((id.to_string(), text.to_string()));
        } else if let Some((id, text)) = line.split_once('|') {
            out.push((id.to_string(), text.to_string()));
        } else {
            // Treat the whole line as text with a synthetic id.
            out.push((format!("row{}", out.len() + 1), line.to_string()));
        }
    }
    Ok(out)
}

// â”€â”€ Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_claim_kind_ok() {
        assert!(parse_claim_kind("repo").is_ok());
        assert!(parse_claim_kind("worktree").is_ok());
        assert!(parse_claim_kind("subproject").is_ok());
        assert!(parse_claim_kind("branch").is_ok());
    }

    #[test]
    fn parse_claim_kind_err() {
        assert!(parse_claim_kind("nope").is_err());
    }

    #[test]
    fn read_id_text_handles_tsv() {
        let dir = std::env::temp_dir().join("agileplus_dedup_test.tsv");
        std::fs::write(&dir, "wp-1\thello world\nwp-2\thello world\n").unwrap();
        let items = read_id_text(dir.to_str().unwrap()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "wp-1");
        assert_eq!(items[1].1, "hello world");
    }

    #[test]
    fn dedup_explain_pair_hybrid_matches_helper() {
        let a = "audit fastapi routes";
        let b = "audit fastapi endpoints";
        let (h, _fj, _nj, _lr, _sd) = hybrid_score(a, b);
        let j = token_jaccard(a, b);
        assert!(h > 0.0);
        assert!(j > 0.0);
    }
}
