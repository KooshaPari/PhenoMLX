//! End-to-end check for [`HuggingFaceAdapter::from_env`] env wiring.

use std::sync::mpsc;
use std::time::Duration;

use hwledger_search_core::source_adapter::SourceAdapter;
use hwledger_search_ingest::HuggingFaceAdapter;

#[test]
fn from_env_reads_hf_token_when_set_and_none_otherwise() {
    let prior_token = std::env::var("HF_TOKEN").ok();

    // Case 1: HF_TOKEN set → adapter carries the token.
    std::env::set_var("HF_TOKEN", "secret-token");
    let a = HuggingFaceAdapter::from_env().expect("from_env ok with token");
    assert_eq!(a.token_snapshot(), Some("secret-token"));

    // Case 2: HF_TOKEN unset → adapter still builds, no token.
    std::env::remove_var("HF_TOKEN");
    let b = HuggingFaceAdapter::from_env().expect("from_env ok without token");
    assert_eq!(b.token_snapshot(), None);

    // Restore prior env so we don't disturb other tests.
    if let Some(v) = prior_token {
        std::env::set_var("HF_TOKEN", v);
    }
}

/// Regression: `list_candidates` must return promptly (not deadlock) when
/// the upstream host is unreachable. Before the fix, `send_with_retry`
/// would acquire `self.last_request` and then call `lock()` again without
/// dropping the first guard when `min_interval_ms` defaulted to 1s but
/// `last_request` was zero — `std::sync::Mutex` is not re-entrant, so
/// the worker self-deadlocked inside `tokio::block_on`.
#[test]
fn list_candidates_returns_promptly_when_upstream_unreachable() {
    let prior_url = std::env::var("HF_HUB_URL").ok();
    std::env::set_var("HF_HUB_URL", "http://127.0.0.1:1");

    let adapter = HuggingFaceAdapter::from_env().expect("from_env ok");
    let (tx, rx) = mpsc::channel::<()>();
    let join = std::thread::spawn(move || {
        // Should return Vec::new() quickly via the "send failed" warning path
        // — never block, never deadlock.
        let _ = adapter.list_candidates(Some("qwen2.5"), 1);
        let _ = tx.send(());
    });

    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("list_candidates deadlocked against unreachable upstream");
        }
        Err(e) => panic!("worker thread panicked before signaling: {e}"),
    }
    join.join().expect("worker thread join");

    if let Some(v) = prior_url {
        std::env::set_var("HF_HUB_URL", v);
    } else {
        std::env::remove_var("HF_HUB_URL");
    }
}
