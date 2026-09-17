//! AgilePlus telemetry — OpenTelemetry traces, metrics, and structured logs.
//!
//! # Quick-start
//!
//! ```no_run
//! use agileplus_telemetry::{TelemetryAdapter, TelemetryConfig, trace_layer};
//! use tracing_subscriber::prelude::*;
//!
//! #[tokio::main]
//! async fn main() {
//!     let cfg = TelemetryConfig::load().unwrap_or_default();
//!     let _adapter = TelemetryAdapter::new(cfg).expect("telemetry init");
//!     tracing_subscriber::registry().with(trace_layer()).init();
//! }
//! ```

pub mod adapter;
pub mod config;
pub mod logs;
pub mod metrics;
pub mod traces;

pub use adapter::{init_telemetry, TelemetryAdapter, TelemetryError, TelemetryGuard};
pub use config::TelemetryConfig;
pub use metrics::{AgilePlusMetrics, MetricsRecorder};
pub use traces::{telemetry_layer, trace_layer};

pub fn init_subscriber() -> Result<TelemetryGuard, TelemetryError> {
    init_telemetry(TelemetryConfig::default())
}
