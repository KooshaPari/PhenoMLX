// SPDX-License-Identifier: MIT OR Apache-2.0
//! Memory profiling tests for `agileplus-domain`.
//!
//! These tests use the [`dhat`] heap profiler to track allocations during
//! core domain operations. They are gated behind the `dhat-heap` feature
//! and are **not** compiled or run during normal `cargo test`.
//!
//! # Usage
//!
//! ```bash
//! cargo test --features dhat-heap \
//!   --manifest-path crates/agileplus-domain/Cargo.toml \
//!   -- -Z unstable-options --report-time
//! ```
//!
//! Heap profiles are written to `dhat-heap*.json` files in the workspace
//! root. These can be uploaded as CI artifacts or inspected locally.
//!
//! # Output
//!
//! Each test that creates a [`dhat::Heap`] profiler will produce a
//! `dhat-heap.json` (or `dhat-heap.N.json` for multiple runs) containing:
//!
//! - Total bytes allocated
//! - Total bytes freed
//! - Maximum heap usage (high-water mark)
//! - Per-backtrace allocation hotspots (when `dhat` is built without `empty`)
//!
//! # Cross-platform
//!
//! [`dhat`] works on any platform Rust supports (Linux, macOS, Windows).
//! For macOS-specific allocation profiling with Instruments, see the
//! `.instruments.yml` config in this directory and run:
//!
//! ```bash
//! cargo instruments --template "Allocation Profiling" \
//!   --manifest-path crates/agileplus-domain/Cargo.toml --test memory
//! ```

#![cfg(feature = "dhat-heap")]

use dhat::{Alloc, Heap};

#[global_allocator]
static ALLOC: Alloc = Alloc;

use agileplus_domain::adapters::noop_trace_adapter::NoopTraceAdapter;
use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::state_machine::FeatureState;
use agileplus_domain::domain::work_package::WorkPackage;
use agileplus_domain::domain::work_package::WpState;
use agileplus_domain::ports::traceability_port::TraceabilityPort;
use agileplus_domain::traceability::TraceRef;
use chrono::Utc;

// ──────────────────────────────────────────────
//  Profile: Feature lifecycle (creation + state transitions)
// ──────────────────────────────────────────────

/// Profile heap usage during creation and state-machine transitions of a
/// large batch of [`Feature`] aggregates.
#[test]
fn memory_profile_feature_lifecycle() {
    let _profiler = Heap::new();

    let mut features: Vec<Feature> = (0..10_000)
        .map(|i| {
            Feature::new(
                &format!("feature-{i}"),
                &format!("Feature {i}"),
                [i as u8; 32],
                None,
            )
        })
        .collect();

    // Walk each feature through the state machine.
    for feature in features.iter_mut() {
        let _ = feature.transition(FeatureState::Specified);
        let _ = feature.transition(FeatureState::Researched);
        let _ = feature.transition(FeatureState::Planned);
    }

    // Serialize + deserialize round-trip.
    let json = serde_json::to_string(&features).expect("serialize features");
    let _round_tripped: Vec<Feature> =
        serde_json::from_str(&json).expect("deserialize features");

    // profiler drops here → writes dhat-heap.json
}

// ──────────────────────────────────────────────
//  Profile: WorkPackage batch operations
// ──────────────────────────────────────────────

/// Profile heap usage during creation, state transitions, and serialization
/// of [`WorkPackage`] values.
#[test]
fn memory_profile_work_package_operations() {
    let _profiler = Heap::new();

    let mut work_packages: Vec<WorkPackage> = (0..10_000)
        .map(|i| {
            WorkPackage::new(
                i as i64 % 100,
                &format!("WP-{i}"),
                i as i32,
                "User can perform the specified operation successfully",
            )
        })
        .collect();

    // Transition some through the workflow.
    for (idx, wp) in work_packages.iter_mut().enumerate() {
        if idx % 5 == 0 {
            wp.state = WpState::Doing;
        } else if idx % 7 == 0 {
            wp.state = WpState::Done;
        }
    }

    // JSON round-trip.
    let json = serde_json::to_string(&work_packages).expect("serialize WPs");
    let _round_tripped: Vec<WorkPackage> =
        serde_json::from_str(&json).expect("deserialize WPs");
}

// ──────────────────────────────────────────────
//  Profile: Traceability bridge operations
// ──────────────────────────────────────────────

/// Profile heap usage during trace-ref linking across many artifacts.
#[test]
fn memory_profile_traceability_bridge() {
    let _profiler = Heap::new();
    let adapter = NoopTraceAdapter;

    let entries: Vec<(TraceRef, String)> = (0..500)
        .map(|i| {
            let trace = TraceRef {
                trace_id: format!("TR-{i}"),
                artifact_type: match i % 3 {
                    0 => "requirement",
                    1 => "test_case",
                    _ => "evidence",
                }
                .to_string(),
                linked_at: Utc::now(),
            };
            let eid = format!("entity-{i}");
            (trace, eid)
        })
        .collect();

    // Link each trace — each call creates a scoped tokio runtime so
    // allocations are captured by dhat.
    for (trace, eid) in &entries {
        let rt = tokio::runtime::Runtime::new().expect("build tokio rt");
        let _ = rt.block_on(adapter.link_trace(eid.clone(), trace.clone()));
    }

    // Retrieval across all entries.
    let rt = tokio::runtime::Runtime::new().expect("build tokio rt");
    let _retrieved = rt.block_on(adapter.get_traces("entity-0".to_string()));
}

// ──────────────────────────────────────────────
//  Profile: JSON serialization stress test
// ──────────────────────────────────────────────

/// Profile heap pressure during repeated serialization of a large domain
/// object graph — a common bottleneck in REST/gRPC handlers.
#[test]
fn memory_profile_json_serialization_stress() {
    let _profiler = Heap::new();

    let features: Vec<Feature> = (0..5_000)
        .map(|i| {
            Feature::new(
                &format!("stress-feature-{i}"),
                &format!("Stress Feature {i}"),
                [42; 32],
                Some("feat/stress"),
            )
        })
        .collect();

    // Serialise and deserialise repeatedly.
    for _ in 0..5 {
        let json = serde_json::to_string(&features).expect("serialize");
        let _: Vec<Feature> = serde_json::from_str(&json).expect("deserialize");
    }
}
