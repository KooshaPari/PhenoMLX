//! Domain types for the AgilePlus event sourcing system.
//!
//! Un-parked 2026-07-04 (operator-authorized): re-declares the domain aggregates
//! that #878 stripped from this file. `cycle` and `work_package` use their richer
//! directory forms (the flat `.rs` duplicates were removed to fix the E0761 that
//! #873 originally addressed and #877 reintroduced).

pub mod api_key;
pub mod audit;
pub mod backlog;
pub mod cycle;
pub mod device_node;
pub mod epic;
pub mod event;
pub mod feature;
pub mod governance;
pub mod metric;
pub mod module;
pub mod project;
pub mod service_health;
pub mod snapshot;
pub mod state_machine;
pub mod story;
pub mod sync_mapping;
pub mod user;
pub mod work_package;

#[cfg(test)]
mod proptest_enums;
