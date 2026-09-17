//! Test fixtures module — re-export from agileplus-fixtures.
//!
//! This module re-exports all fixture functionality from the shared agileplus-fixtures
//! crate. The actual implementations have been extracted to enable reuse across multiple
//! test suites.
//!
//! Traceability: WP19-T107

pub use agileplus_fixtures::{
    FeatureBuilder, TestFixtures, WorkPackageBuilder, feature_create_payload,
    plane_webhook_payload, seed_test_data, transition_payload,
};
