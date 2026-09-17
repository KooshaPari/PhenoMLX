// SPDX-License-Identifier: MIT OR Apache-2.0
//! AgilePlus protobuf/tonic generated types.
//!
//! When `protoc` is available at build time `build.rs` compiles the `.proto`
//! files and the output is included via the `include!` macro below.  When
//! `protoc` is absent (CI check-only, dev machines without the compiler)
//! `build.rs` emits `cargo:rustc-cfg=agileplus_proto_stubs` and the
//! hand-written stubs module is compiled instead so `cargo check --workspace`
//! stays green.
//!
//! Traceability: FR-AGP-011

pub mod agileplus {
    pub mod v1 {
        // Include protoc output when available …
        #[cfg(not(agileplus_proto_stubs))]
        include!(concat!(env!("OUT_DIR"), "/agileplus.v1.rs"));

        // … otherwise use the hand-written stubs.
        #[cfg(agileplus_proto_stubs)]
        include!("stubs.rs");
    }
}

#[cfg(test)]
mod tests {
    use super::agileplus::v1::*;

    #[test]
    fn test_feature_state_default_and_construction() {
        // Test that proto types can be constructed via Default
        let state = FeatureState::default();
        assert_eq!(state.state, "");
        assert_eq!(state.next_command, "");
        assert!(state.blockers.is_empty());
        assert!(state.governance.is_none());

        // Test explicit construction
        let state = FeatureState {
            state: "InProgress".to_string(),
            next_command: "continue".to_string(),
            blockers: vec!["blocker1".to_string()],
            governance: Some(GovernanceSummary {
                gate_passed: true,
                violations_count: 0,
            }),
        };
        assert_eq!(state.state, "InProgress");
        assert_eq!(state.next_command, "continue");
        assert_eq!(state.blockers.len(), 1);
        assert!(state.governance.is_some());
    }

    #[test]
    fn test_work_package_status_conversion_and_clone() {
        // Test conversion/utility: Clone, PartialEq, and field access
        let wp = WorkPackageStatus {
            id: 1,
            title: "WP-1".to_string(),
            state: "Done".to_string(),
            sequence: 1,
            agent_id: "agent-1".to_string(),
            pr_url: "https://github.com/test/repo/pull/1".to_string(),
            pr_state: "merged".to_string(),
            depends_on: vec![0],
            file_scope: vec!["src/main.rs".to_string()],
        };

        // Test Clone
        let wp_cloned = wp.clone();
        assert_eq!(wp, wp_cloned);

        // Test PartialEq with different instance
        let wp2 = WorkPackageStatus {
            id: 1,
            title: "WP-1".to_string(),
            state: "Done".to_string(),
            sequence: 1,
            agent_id: "agent-1".to_string(),
            pr_url: "https://github.com/test/repo/pull/1".to_string(),
            pr_state: "merged".to_string(),
            depends_on: vec![0],
            file_scope: vec!["src/main.rs".to_string()],
        };
        assert_eq!(wp, wp2);

        // Test modification breaks equality
        let mut wp3 = wp.clone();
        wp3.state = "InProgress".to_string();
        assert_ne!(wp, wp3);

        // Test Debug formatting works
        let debug_str = format!("{:?}", wp);
        assert!(debug_str.contains("WP-1"));
        assert!(debug_str.contains("Done"));
    }

    // Serialization/deserialization tests only run when protoc-generated code is available
    // (the hand-written stubs don't implement prost::Message)
    #[cfg(not(agileplus_proto_stubs))]
    mod prost_tests {
        use super::*;
        use prost::Message;

        #[test]
        fn test_feature_serialize_deserialize() {
            let feature = Feature {
                id: 42,
                slug: "test-feature".to_string(),
                friendly_name: "Test Feature".to_string(),
                state: "Draft".to_string(),
                target_branch: "feature/test".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-02T00:00:00Z".to_string(),
                wp_count: 5,
                wp_done: 2,
            };

            let mut buf = Vec::new();
            feature.encode(&mut buf).expect("encoding should succeed");
            assert!(!buf.is_empty());

            let decoded = Feature::decode(&buf[..]).expect("decoding should succeed");

            assert_eq!(feature, decoded);
            assert_eq!(decoded.id, 42);
            assert_eq!(decoded.slug, "test-feature");
            assert_eq!(decoded.friendly_name, "Test Feature");
            assert_eq!(decoded.wp_count, 5);
            assert_eq!(decoded.wp_done, 2);
        }

        #[test]
        fn test_backlog_item_roundtrip() {
            let item = BacklogItemProto {
                id: 100,
                title: "Backlog Item".to_string(),
                description: "Description here".to_string(),
                r#type: "Story".to_string(),
                priority: "High".to_string(),
                status: "Open".to_string(),
                source: "github".to_string(),
                feature_slug: "feature-x".to_string(),
                tags: vec!["tag1".to_string(), "tag2".to_string()],
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            };

            let mut buf = Vec::new();
            item.encode(&mut buf).expect("encoding should succeed");
            let decoded = BacklogItemProto::decode(&buf[..]).expect("decoding should succeed");

            assert_eq!(item, decoded);
            assert_eq!(decoded.id, 100);
            assert_eq!(decoded.tags.len(), 2);
        }

        #[test]
        fn test_command_response_with_hashmap() {
            use std::collections::HashMap;
            let mut outputs = HashMap::new();
            outputs.insert("key1".to_string(), "value1".to_string());
            outputs.insert("key2".to_string(), "value2".to_string());

            let resp = CommandResponse {
                success: true,
                message: "OK".to_string(),
                outputs,
            };

            let mut buf = Vec::new();
            resp.encode(&mut buf).expect("encoding should succeed");
            let decoded = CommandResponse::decode(&buf[..]).expect("decoding should succeed");

            assert_eq!(resp.success, decoded.success);
            assert_eq!(resp.message, decoded.message);
            assert_eq!(decoded.outputs.len(), 2);
            assert_eq!(decoded.outputs.get("key1"), Some(&"value1".to_string()));
        }
    }
}
