//! AgilePlus CLI sub-commands.
//!
//! Default build exports **platform** only (real health probes). Enable the
//! `full` feature for dashboard/events/sync/audit/registry when those deps
//! are available in the workspace.
//!
//! Traceability: FR-048, FR-049 / WP14 / WP20

#[cfg(feature = "audit")]
pub mod audit;
#[cfg(feature = "dashboard")]
pub mod dashboard;
#[cfg(feature = "events")]
pub mod events;
#[cfg(feature = "platform")]
pub mod platform;
#[cfg(feature = "registry")]
pub mod registry;
#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "audit")]
pub use audit::AuditLog;
#[cfg(feature = "dashboard")]
pub use dashboard::{
    DashboardArgs, DashboardOpenArgs, DashboardPortArgs, DashboardSubcommand, api_reachable,
    configured_port, dashboard_url, run_dashboard, run_dashboard_open, run_dashboard_port,
};
#[cfg(feature = "events")]
pub use events::{
    EventOutputFormat, EventQueryResult, EventRecord, EventsArgs, filter_events, parse_since,
    render_json, render_jsonl, render_table, run_events,
};
#[cfg(feature = "platform")]
pub use platform::{
    OverallStatus, PlatformArgs, PlatformDownArgs, PlatformHealth, PlatformLogsArgs,
    PlatformStatusArgs, PlatformSubcommand, PlatformUpArgs, ServiceHealth, ServiceStatus,
    run_platform, run_platform_down, run_platform_logs, run_platform_status, run_platform_up,
};
#[cfg(feature = "registry")]
pub use registry::{SubCommand, SubCommandCategory, SubCommandRegistry};
#[cfg(feature = "sync")]
pub use sync::{
    AutoSyncAction, ConflictResolution, SyncArgs, SyncAutoArgs, SyncConfig, SyncConflict,
    SyncDirection, SyncItemOutcome, SyncPullArgs, SyncPushArgs, SyncReport, SyncReportEntry,
    SyncResolveArgs, SyncStatusArgs, SyncStatusRow, SyncSubcommand, run_sync,
};
