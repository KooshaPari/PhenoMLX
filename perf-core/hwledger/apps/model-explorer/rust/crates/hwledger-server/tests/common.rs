//! Shared helpers for the integration tests in `tests/`.
//!
//! Each test file declares `mod common;` and uses [`common::seeded_app`]
//! to spin up an in-process tantivy fixture + the same axum router the
//! binary exposes, then drives it with `tower::ServiceExt::oneshot`.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response};
use bytes::Bytes;
use http_body_util::BodyExt;
use hwledger_search_index::{IndexedDoc, TantivyStore};
use tempfile::TempDir;

use hwledger_server::{router, AppState};

/// In-process tantivy store + directory + router.
///
/// `TempDir` keeps the directory alive for the lifetime of the test; when
/// `App` drops, the directory is removed.
pub struct App {
    /// Temp directory holding the tantivy store. Held only to keep the
    /// path alive; tests shouldn't read it directly.
    #[allow(dead_code)]
    pub dir: TempDir,
    /// Resolved data dir path (passed to `AppState`).
    pub data_dir: PathBuf,
    /// The mounted axum router.
    pub router: axum::Router,
}

/// Spin up a fresh tantivy fixture with three real models and mount the
/// server router against it.
#[must_use]
pub fn seeded_app() -> App {
    seed_with_models(&[
        (
            "hf::qwen/Qwen2.5-7B-Instruct",
            "Qwen2.5 7B Instruct",
            "qwen",
            "instruct",
            "qwen2",
            "gqa",
            "gguf gptq",
            "Qwen2.5 is the latest series of large language models from Alibaba.",
        ),
        (
            "hf::meta-llama/Llama-3-8B-Instruct",
            "Llama 3 8B Instruct",
            "meta-llama",
            "instruct",
            "llama",
            "gqa",
            "gguf",
            "Meta's Llama 3 instruction-tuned model.",
        ),
        (
            "hf::mistralai/Mistral-7B-Instruct-v0.3",
            "Mistral 7B Instruct v0.3",
            "mistralai",
            "instruct",
            "mistral",
            "sma",
            "gguf gptq awq",
            "Mistral 7B base fine-tuned for instruction following.",
        ),
    ])
}

/// Spin up a fixture that has at least one model whose `kind` is `agentic`
/// and one whose `kind` is `coding`, in addition to the standard instruct
/// rows. Used by the [`AgenticFitRerank`](hwledger_search_skills::AgenticFitRerank)
/// integration tests so the per-result intent-fit payload the
/// [`service::search_results`](hwledger_server::service::search_results)
/// projects is non-trivial for both intents.
#[must_use]
pub fn seeded_app_with_use_case_kinds() -> App {
    seed_with_models(&[
        (
            "hf::qwen/Qwen2.5-7B-Instruct",
            "Qwen2.5 7B Instruct",
            "qwen",
            "instruct",
            "qwen2",
            "gqa",
            "gguf gptq",
            "Qwen2.5 is the latest series of large language models from Alibaba.",
        ),
        (
            "hf::meta-llama/Llama-3-8B-Instruct",
            "Llama 3 8B Instruct",
            "meta-llama",
            "instruct",
            "llama",
            "gqa",
            "gguf",
            "Meta's Llama 3 instruction-tuned model.",
        ),
        (
            "hf::mistralai/Mistral-7B-Instruct-v0.3",
            "Mistral 7B Instruct v0.3",
            "mistralai",
            "instruct",
            "mistral",
            "sma",
            "gguf gptq awq",
            "Mistral 7B base fine-tuned for instruction following.",
        ),
        (
            "hf::agent-org/Tool-Use-Agent",
            "Tool-Use Agent",
            "agent-org",
            "agentic",
            "agentic",
            "gqa",
            "gguf",
            "Agent-style model with strong tool calling capabilities.",
        ),
        (
            "hf::coder-org/CodeLlama-7B",
            "CodeLlama 7B",
            "coder-org",
            "coding",
            "llama",
            "gqa",
            "gguf",
            "Code completion model fine-tuned for programming tasks.",
        ),
    ])
}

fn seed_with_models(rows: &[(&str, &str, &str, &str, &str, &str, &str, &str)]) -> App {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TantivyStore::open(dir.path()).expect("open tantivy");
    for row in rows {
        let (id, name, org, kind, family, arch, quants, card_snippet) = *row;
        store
            .upsert(&IndexedDoc {
                id,
                name,
                org,
                kind,
                family,
                arch,
                quants,
                card_snippet,
            })
            .expect("upsert");
    }
    store.commit().expect("commit");

    let data_dir = dir.path().to_path_buf();
    let state = AppState::new(Arc::new(store), data_dir.clone());
    let router = router(state);
    App {
        dir,
        data_dir,
        router,
    }
}

/// Drive a request through the in-process router and return the
/// `Response<Body>` plus the buffered body bytes.
pub async fn send(app: axum::Router, req: Request<Body>) -> (Response<Body>, Bytes) {
    use tower::ServiceExt;
    let resp = app.oneshot(req).await.expect("oneshot");
    let (parts, body) = resp.into_parts();
    let bytes = body.collect().await.expect("collect body").to_bytes();
    (Response::from_parts(parts, Body::empty()), bytes)
}