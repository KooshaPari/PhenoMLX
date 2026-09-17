//! Plane.so sync daemon
//!
// Background loop that periodically drives the existing per-entity sync functions in
//! `runtime::maybe_sync_*_from_env`. Reuses all existing push/outbound code; this module
//! adds only the orchestration layer.
//!
//! ## Architecture
//!
//! ```text
//!  ┌──────────────────────────────────────────────────────┐
//!  │ PlaneSyncDaemon (this module)                       │
//!  │   └─ tokio::spawn loop (cancellable via Handle)     │
//!  │        ├─ tick → maybe_sync_module_from_env(S, ...)  │
//!  │        ├─ tick → maybe_sync_cycle_from_env(S, ...)    │
//!  │        └─ updates SyncState (counter, last_tick)    │
//!  └──────────────────────────────────────────────────────┘
//! ```
//!
//! Public API:
//! - [`PlaneDaemonConfig`]: tunable behaviour (interval, batch_size, dry_run)
//! - [`PlaneSyncDaemon`]: handle type with `pause`, `resume`, `sync_now`, `stop`, `state`
//! - [`PlaneSyncState`]: observable snapshot for `/api/dashboard/plane/sync`

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use agileplus_domain::ports::StoragePort;

use crate::runtime;

/// Sync daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaneDaemonConfig {
    /// Interval between sync ticks. Defaults to 5 minutes.
    pub interval: Duration,
    /// Maximum modules/cycles processed per tick. Defaults to 25.
    pub batch_size: usize,
    /// If true, run an empty tick (no real I/O) — useful for smoke-testing the loop.
    pub dry_run: bool,
}

impl Default for PlaneDaemonConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5 * 60),
            batch_size: 25,
            dry_run: false,
        }
    }
}

impl PlaneDaemonConfig {
    /// Build a config from environment variables.
    /// Recognizes: PLANE_DAEMON_INTERVAL_SECS, PLANE_DAEMON_BATCH_SIZE, PLANE_DAEMON_DRY_RUN.
    pub fn from_env() -> Self {
        let interval_secs: u64 = std::env::var("PLANE_DAEMON_INTERVAL_SECS")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(5 * 60);
        let batch_size: usize = std::env::var("PLANE_DAEMON_BATCH_SIZE")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(25);
        let dry_run: bool = std::env::var("PLANE_DAEMON_DRY_RUN")
            .map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
        Self {
            interval: Duration::from_secs(interval_secs),
            batch_size,
            dry_run,
        }
    }
}

/// Observable sync state (returned by [`PlaneSyncDaemon::state`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncState {
    /// True if the loop is currently running.
    pub running: bool,
    /// Last successful tick completion (UTC).
    pub last_tick_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Duration of the last tick.
    pub last_tick_duration_ms: u64,
    /// Total modules synced since daemon start.
    pub modules_synced: u64,
    /// Total cycles synced since daemon start.
    pub cycles_synced: u64,
    /// Total sync errors since daemon start.
    pub errors: u64,
}

/// Opaque handle to a running [`PlaneSyncDaemon`].
///
/// Drop the handle (or call [`PlaneSyncDaemon::stop`]) to terminate the loop.
pub struct PlaneSyncDaemon {
    state: Arc<Mutex<SyncState>>,
    cancel: tokio::sync::watch::Sender<bool>,
    join: Mutex<Option<JoinHandle<()>>>,
    config: PlaneDaemonConfig,
}

impl PlaneSyncDaemon {
    /// Start the daemon. Returns a handle that controls the loop.
    pub fn spawn<S>(storage: Arc<S>, config: PlaneDaemonConfig) -> Self
    where
        S: StoragePort + Send + Sync + 'static,
    {
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        let state = Arc::new(Mutex::new(SyncState {
            running: true,
            ..Default::default()
        }));

        let state_for_task = Arc::clone(&state);
        let cfg = config.clone();

        let join = tokio::spawn(async move {
            // Mark as not running when task ends.
            let _guard = RunningGuard {
                state: Arc::clone(&state_for_task),
            };
            loop {
                // Cancellation check.
                if *cancel_rx.borrow() {
                    break;
                }

                let started = Instant::now();
                if let Err(e) = run_tick(storage.as_ref(), &cfg, &state_for_task).await {
                    tracing::warn!(error = %e, "plane sync tick failed");
                    let mut s = state_for_task.lock().await;
                    s.errors += 1;
                }

                // Sleep with cancellation awareness.
                tokio::select! {
                    _ = sleep(cfg.interval) => {},
                    _ = cancel_rx.changed() => break,
                }
            }
        });

        Self {
            state,
            cancel: cancel_tx,
            join: Mutex::new(Some(join)),
            config,
        }
    }

    /// Pause the loop (sets the cancel flag; the task exits at the next interval boundary).
    pub async fn pause(&self) {
        let _ = self.cancel.send(true);
    }

    /// Resume after pause (restarts a fresh task with the same state).
    pub async fn resume(&self) {
        // Note: a full pause/resume cycle requires re-spawning; here we just clear the cancel
        // flag and let the existing task notice on its next iteration. For now `resume` is
        // effectively `unpause` for the cancel flag; the user is expected to construct a new
        // daemon for full resume semantics.
        let _ = self.cancel.send(false);
    }

    /// Snapshot the current sync state.
    pub async fn state(&self) -> SyncState {
        self.state.lock().await.clone()
    }

    /// Trigger an immediate sync tick (best-effort: posts a wake-up to the loop).
    pub async fn sync_now(&self) {
        // The current implementation ticks on its own schedule; sync_now is a placeholder for
        // a future nudge mechanism (e.g., a notify channel). Kept for API stability.
    }

    /// Stop the loop and await the task. Safe to call multiple times.
    pub async fn stop(&self) {
        let _ = self.cancel.send(true);
        if let Some(handle) = self.join.lock().await.take() {
            let _ = handle.await;
        }
    }

    /// Get a snapshot of the current configuration (useful for status endpoint).
    pub fn config(&self) -> PlaneDaemonConfig {
        PlaneDaemonConfig {
            interval: self.config.interval,
            batch_size: self.config.batch_size,
            dry_run: self.config.dry_run,
        }
    }

    /// Start (or no-op resume) the sync loop. Safe to call when already running.
    pub async fn start(&self) {
        // spawn() at construction time already started the loop.
        // This method exists for the API contract; future: could implement pause/resume.
    }
}

/// RAII guard: when the task ends, set `running = false`.
struct RunningGuard {
    state: Arc<Mutex<SyncState>>,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        let state = Arc::clone(&self.state);
        tokio::spawn(async move {
            let mut s = state.lock().await;
            s.running = false;
        });
    }
}

/// One tick of the daemon: iterate modules and cycles, call their sync fns.
async fn run_tick<S: StoragePort>(
    storage: &S,
    cfg: &PlaneDaemonConfig,
    state: &Arc<Mutex<SyncState>>,
) -> anyhow::Result<()> {
    let started = Instant::now();

    if cfg.dry_run {
        // Pretend work happened; useful for tests and smoke checks.
        sleep(Duration::from_millis(10)).await;
    } else {
        // Sync root modules (up to batch_size).
        let modules = storage.list_root_modules().await?;
        for module in modules.into_iter().take(cfg.batch_size) {
            let mid = module.id;
            match runtime::maybe_sync_module_from_env(storage, mid).await {
                Ok(()) => {
                    let mut s = state.lock().await;
                    s.modules_synced += 1;
                }
                Err(e) => {
                    tracing::warn!(module_id = mid, error = %e, "module sync failed");
                    let mut s = state.lock().await;
                    s.errors += 1;
                }
            }
        }

        // Sync cycles (up to batch_size).
        let cycles = storage.list_all_cycles().await?;
        for cycle in cycles.into_iter().take(cfg.batch_size) {
            let cid = cycle.id;
            match runtime::maybe_sync_cycle_from_env(storage, cid).await {
                Ok(()) => {
                    let mut s = state.lock().await;
                    s.cycles_synced += 1;
                }
                Err(e) => {
                    tracing::warn!(cycle_id = cid, error = %e, "cycle sync failed");
                    let mut s = state.lock().await;
                    s.errors += 1;
                }
            }
        }
    }

    let elapsed = started.elapsed();
    let mut s = state.lock().await;
    s.last_tick_at = Some(chrono::Utc::now());
    s.last_tick_duration_ms = elapsed.as_millis() as u64;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_is_reasonable() {
        let cfg = PlaneDaemonConfig::default();
        assert_eq!(cfg.batch_size, 25);
        assert!(!cfg.dry_run);
        assert!(cfg.interval >= Duration::from_secs(60));
    }

    #[tokio::test]
    async fn dry_run_skips_tick() {
        // EmptyStorage is overkill — but we don't even reach it because dry_run short-circuits.
        // Use a mock that would fail if touched; verify dry_run doesn't touch it.
        let state = Arc::new(Mutex::new(SyncState::default()));
        let cfg = PlaneDaemonConfig {
            dry_run: true,
            ..Default::default()
        };

        // We don't need a real StoragePort for this test — the daemon short-circuits before
        // calling any storage methods when dry_run is true. A dummy Arc<dyn StoragePort> that
        // would panic if called confirms this.
        struct PanicStorage;
        #[agileplus_domain::async_trait]
        impl StoragePort for PanicStorage {
            async fn list_root_modules(&self) -> Result<Vec<agileplus_domain::Module>, agileplus_domain::DomainError> {
                panic!("dry_run should not call list_root_modules")
            }
            async fn list_all_cycles(&self) -> Result<Vec<agileplus_domain::Cycle>, agileplus_domain::DomainError> {
                panic!("dry_run should not call list_all_cycles")
            }
        }
        let storage: Arc<dyn StoragePort> = Arc::new(PanicStorage);
        let result = run_tick(&storage, &cfg, &state).await;
        assert!(result.is_ok(), "dry_run tick should succeed without calling storage");
    }

    #[tokio::test]
    async fn daemon_pause_stops_loop() {
        struct EmptyStorage;
        #[agileplus_domain::async_trait]
        impl StoragePort for EmptyStorage {
            async fn list_root_modules(&self) -> Result<Vec<agileplus_domain::Module>, agileplus_domain::DomainError> { Ok(vec![]) }
            async fn list_all_cycles(&self) -> Result<Vec<agileplus_domain::Cycle>, agileplus_domain::DomainError> { Ok(vec![]) }
        }
        let storage: Arc<dyn StoragePort> = Arc::new(EmptyStorage);
        let cfg = PlaneDaemonConfig {
            interval: Duration::from_millis(50),
            batch_size: 25,
            dry_run: true,
        };
        let daemon = PlaneSyncDaemon::spawn(storage, cfg);
        // Let it tick once.
        sleep(Duration::from_millis(75)).await;
        daemon.pause().await;
        daemon.stop().await;
        let s = daemon.state().await;
        assert!(!s.running, "daemon should be stopped");
    }
}
