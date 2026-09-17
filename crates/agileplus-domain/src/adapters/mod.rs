//! Hexagonal-architecture adapter implementations.
//!
//! Each sub-module provides a concrete implementation of a
//! [`TraceabilityPort`](crate::ports::traceability_port::TraceabilityPort) or another
//! domain port. Adapters are swappable — the application is composed by injecting
//! the desired implementation at startup.

pub mod noop_trace_adapter;
