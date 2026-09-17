//! tracing-subscriber setup with sensible defaults.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize tracing-subscriber with the given verbosity level.
///
/// Honors `RUST_LOG` env var if set; otherwise uses the supplied
/// `filter` (typically `Verbosity::to_filter()` output).
///
/// Idempotent: safe to call from tests because `init()` will silently
/// succeed if a global subscriber is already set. We swallow the
/// `SetGlobalDefaultError` so library code can be called multiple times.
pub fn setup_tracing(filter: tracing_subscriber::filter::LevelFilter) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::default().add_directive(filter.into()));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_target(false))
        .try_init();
}

/// Initialize tracing-subscriber from a verbosity-count integer.
///
/// Convenience for `ArgAction::Count`-style CLIs.
pub fn setup_tracing_from_count(verbose_count: u8, quiet: bool) {
    let filter = if quiet {
        tracing_subscriber::filter::LevelFilter::ERROR
    } else {
        match verbose_count {
            0 => tracing_subscriber::filter::LevelFilter::INFO,
            1 => tracing_subscriber::filter::LevelFilter::DEBUG,
            _ => tracing_subscriber::filter::LevelFilter::TRACE,
        }
    };
    setup_tracing(filter);
}
