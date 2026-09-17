//! Claim engine throughput benchmark (STUB — wire in Cargo.toml before running).
//!
//! Measures hot paths in `agileplus_triage::claim::ClaimStore`:
//!   - `claim` issue under contention
//!   - `lookup` by resource
//!   - `heartbeat` refresh
//!   - `reap_expired` full-store sweep
//!
//! Target (proposed): ≥5,000 claims/sec in-memory at p95.
//!
//! Registration: see `benches/BENCH_REGISTRATION.snippet.toml`.
//!
//! Run (after wiring):
//!   cargo bench -p agileplus-benchmarks --bench claim_engine_perf

#![allow(dead_code, unused_imports)]

use std::hint::black_box;

use agileplus_triage::claim::{ClaimKind, ClaimReason, ClaimStore};
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn seed_store(n: usize) -> ClaimStore {
    let mut store = ClaimStore::new();
    for i in 0..n {
        let id = format!("claim-{i}");
        let resource = format!("repo:bench-{i}");
        store.claim(
            &id,
            &resource,
            ClaimKind::Repo,
            "bench-agent",
            3600,
            ClaimReason::WipRun("bench".into()),
        );
    }
    store
}

fn bench_claim_issue(c: &mut Criterion) {
    let mut group = c.benchmark_group("claim_engine_issue");

    for count in [100_usize, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("sequential", count), &count, |b, &n| {
            b.iter(|| {
                let mut store = ClaimStore::new();
                for i in 0..n {
                    let id = format!("c-{i}");
                    let resource = format!("repo:{i}");
                    black_box(store.claim(
                        &id,
                        &resource,
                        ClaimKind::Repo,
                        "agent",
                        3600,
                        ClaimReason::default(),
                    ));
                }
            });
        });
    }

    group.finish();
}

fn bench_lookup(c: &mut Criterion) {
    let store = seed_store(10_000);
    let mut group = c.benchmark_group("claim_engine_lookup");

    group.bench_function("lookup_existing_10k", |b| {
        b.iter(|| {
            for i in 0..1_000 {
                let resource = format!("repo:bench-{i}");
                black_box(store.lookup(ClaimKind::Repo, &resource));
            }
        });
    });

    group.finish();
}

fn bench_heartbeat(c: &mut Criterion) {
    let mut store = seed_store(1_000);
    let ids: Vec<String> = (0..1_000).map(|i| format!("claim-{i}")).collect();

    c.bench_function("heartbeat_1000_rounds", |b| {
        b.iter(|| {
            for id in &ids {
                black_box(store.heartbeat(id));
            }
        });
    });
}

fn bench_reap_expired(c: &mut Criterion) {
    let mut group = c.benchmark_group("claim_engine_reap");

    for count in [100_usize, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("expired_sweep", count), &count, |b, &n| {
            b.iter(|| {
                let mut store = ClaimStore::new();
                for i in 0..n {
                    let id = format!("c-{i}");
                    let resource = format!("repo:{i}");
                    store.claim(
                        &id,
                        &resource,
                        ClaimKind::Repo,
                        "agent",
                        1, // 1-second TTL — all expired when we reap
                        ClaimReason::default(),
                    );
                }
                // Simulate elapsed TTL
                let now = Utc::now() + chrono::Duration::seconds(2);
                black_box(store.reap_expired(now))
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_claim_issue,
    bench_lookup,
    bench_heartbeat,
    bench_reap_expired
);
criterion_main!(benches);
