// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Fuzz target for `agileplus-domain` identifier / code / string parsers.
//
// Exercises every FromStr implementation, domain constructor with string
// validation, slug generator, and JSON deserialization path in the crate.
// Catches panics, unexpected unwrap() calls, and integer overflow in:
//   - slug / name / email validation   - FromStr for all enum types
//   - Project, Story, Epic, User constructors   - JSON hex_bytes deserializer
//   - serde_json deserialization of every domain aggregate

#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        use agileplus_domain::domain::{
            api_key::ApiKey,
            backlog::{BacklogItem, BacklogPriority, BacklogStatus, Intent},
            cycle::CycleState,
            epic::{Epic, EpicStatus},
            feature::Feature,
            module::Module,
            project::Project,
            state_machine::FeatureState,
            story::{Story, StoryStatus},
            user::{User, UserRole, UserStatus},
            work_package::WorkPackage,
        };

        // ---- Project slug / name validation ----------------------------------
        // Project::new() validates that name is non-empty after trim and that
        // slug is non-empty and matches [a-z0-9-].
        let _ = Project::new(s, "test");
        let _ = Project::new("Test Name", s);

        // ---- Story title validation ------------------------------------------
        // Story::new() rejects empty (post-trim) titles.
        let _ = Story::new(1, 1, s, None);

        // ---- Epic title validation -------------------------------------------
        // Epic::new() rejects empty (post-trim) titles.
        let _ = Epic::new(1, s);

        // ---- User display-name and email validation --------------------------
        // User::new() rejects empty display_name and emails missing '@'.
        let _ = User::new(s, "test@example.com", UserRole::Member);
        let _ = User::new("Test", s, UserRole::Member);

        // ---- FromStr for every domain enum type ------------------------------
        // Each parses a lowercase variant name from the fuzzed string.
        let _ = StoryStatus::from_str(s);
        let _ = EpicStatus::from_str(s);
        let _ = UserRole::from_str(s);
        let _ = UserStatus::from_str(s);
        let _ = CycleState::from_str(s);
        let _ = FeatureState::from_str(s);
        let _ = Intent::from_str(s);
        let _ = BacklogPriority::from_str(s);
        let _ = BacklogStatus::from_str(s);

        // ---- Slug generation (always succeeds, processes arbitrary input) ----
        Project::slug_from_name(s);
        Module::slug_from_name(s);

        // ---- Feature construction --------------------------------------------
        // Feature::new() takes slug, friendly_name, spec_hash and target_branch.
        Feature::new(s, s, [0u8; 32], None);

        // ---- WorkPackage construction ----------------------------------------
        WorkPackage::new(1, s, 0, s);

        // ---- JSON deserialization of domain types ----------------------------
        // Every domain aggregate derives Deserialize; fuzzing from arbitrary
        // JSON exercises serde_json internals with all our custom deserializers.
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<Project>(s) {}
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<Story>(s) {}
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<Feature>(s) {}
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<Epic>(s) {}
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<User>(s) {}
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<WorkPackage>(s) {}
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<BacklogItem>(s) {}

        // ---- Hex-bytes deserializer (via ApiKey) ------------------------------
        // ApiKey.key_hash uses #[serde(with = "hex_bytes")] which calls
        // hex::decode() and validates output length == 32.  Fuzzing arbitrary
        // JSON strings exercises the hex decoder with pathological inputs.
        #[allow(unused_variables)]
        if let Ok(_) = serde_json::from_str::<ApiKey>(s) {}
    }
});
