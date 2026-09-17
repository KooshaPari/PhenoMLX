//! Trace-matrix derivation benchmark (STUB — wire in Cargo.toml before running).
//!
//! Synthetic workload mirroring FR-024-5 auto-matrix pipeline:
//!   1. Parse FR ids from a markdown bullet list
//!   2. Deserialize per-FR trace JSON (5-layer schema)
//!   3. Roll up layer presence into status rows
//!
//! Target (proposed): ≤2s p95 for 200 FRs (cold); ≤200ms with cache key hit.
//!
//! Registration: see `benches/BENCH_REGISTRATION.snippet.toml`.
//!
//! Run (after wiring):
//!   cargo bench -p agileplus-benchmarks --bench trace_matrix_derivation

#![allow(dead_code)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TraceLayer {
    path: String,
}

#[derive(Debug, Deserialize)]
struct TraceFile {
    fr_id: String,
    spec_slug: String,
    docs_pages: Vec<TraceLayer>,
    tests: Vec<TraceLayer>,
    code_modules: Vec<TraceLayer>,
    journeys: Vec<TraceLayer>,
}

#[derive(Debug)]
struct MatrixRow {
    fr_id: String,
    docs_ok: bool,
    tests_ok: bool,
    code_ok: bool,
    journeys_ok: bool,
}

fn parse_fr_list(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("- ")
                .map(|rest| rest.split_whitespace().next().unwrap_or("").to_string())
        })
        .filter(|id| id.starts_with("FR-"))
        .collect()
}

fn make_trace_json(fr_id: &str) -> String {
    format!(
        r#"{{
  "fr_id": "{fr_id}",
  "spec_slug": "eco-024",
  "spec_anchor": "#anchor",
  "docs_pages": [{{"path": "docs/foo.md"}}],
  "tests": [{{"path": "crates/foo/tests/bar.rs"}}],
  "code_modules": [{{"path": "crates/foo/src/lib.rs"}}],
  "journeys": [{{"path": "docs/operations/journeys/{fr_id}.md"}}]
}}"#
    )
}

fn derive_matrix(fr_ids: &[String]) -> Vec<MatrixRow> {
    fr_ids
        .iter()
        .map(|fr_id| {
            let json = make_trace_json(fr_id);
            let trace: TraceFile =
                serde_json::from_str(&json).expect("synthetic trace must parse");
            MatrixRow {
                fr_id: trace.fr_id,
                docs_ok: !trace.docs_pages.is_empty(),
                tests_ok: !trace.tests.is_empty(),
                code_ok: !trace.code_modules.is_empty(),
                journeys_ok: !trace.journeys.is_empty(),
            }
        })
        .collect()
}

fn make_fr_markdown(count: usize) -> String {
    (0..count)
        .map(|i| format!("- FR-024-{i} traceability row"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn bench_matrix_derivation(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace_matrix_derivation");

    for fr_count in [50_usize, 200, 500] {
        let markdown = make_fr_markdown(fr_count);

        group.bench_with_input(
            BenchmarkId::new("cold_derive", fr_count),
            &markdown,
            |b, md| {
                b.iter(|| {
                    let fr_ids = parse_fr_list(md);
                    black_box(derive_matrix(&fr_ids))
                });
            },
        );
    }

    group.finish();
}

fn bench_parse_fr_list_only(c: &mut Criterion) {
    let markdown = make_fr_markdown(200);

    c.bench_function("parse_fr_list_200", |b| {
        b.iter(|| black_box(parse_fr_list(&markdown)));
    });
}

criterion_group!(benches, bench_matrix_derivation, bench_parse_fr_list_only);
criterion_main!(benches);
