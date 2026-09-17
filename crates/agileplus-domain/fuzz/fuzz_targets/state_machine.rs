// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Fuzz target for `agileplus-domain` FeatureState state machine transitions.
//
// Exercises every code path in the state machine, including:
//   - FeatureState::from_str() parsing (all 8 variants + invalid strings)
//   - FeatureState::transition() forward (valid) and backward (invalid) paths
//   - FeatureState::transition() skip / jump paths (e.g. Created → Shipped)
//   - FeatureState::transition() self-transition (every state → itself)
//   - Feature::transition() aggregate-level wrapper
//   - FeatureState Display round-trip (to_string → from_str)
//
// The fuzzer uses fuzzer-supplied bytes as indices into the 8-variant enum to
// generate source / target state pairs, covering 8×8 = 64 possible transitions
// plus arbitrary garbage string inputs from the raw byte slice.

#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::state_machine::FeatureState;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // ---- FeatureState::from_str: exercise all variant names ----
        // The parser is case-sensitive and expects lowercase variant names.
        #[allow(unused_variables)]
        if let Ok(state) = FeatureState::from_str(s) {
            // Successfully parsed a valid FeatureState from fuzzed input.
        }

        // ---- FeatureState::transition: promote / demote / jump / self ----
        // Use first two bytes as indices into the FeatureState enum (8 variants).
        // This exercises every valid forward transition + every invalid
        // backward / skip / self-transition.
        if data.len() >= 2 {
            let variants = [
                FeatureState::Created,
                FeatureState::Specified,
                FeatureState::Researched,
                FeatureState::Planned,
                FeatureState::Implementing,
                FeatureState::Validated,
                FeatureState::Shipped,
                FeatureState::Retrospected,
            ];

            let from_idx = (data[0] as usize) % variants.len();
            let to_idx = (data[1] as usize) % variants.len();
            let from = variants[from_idx];
            let to = variants[to_idx];

            // Direct transition (state_machine::FeatureState)
            #[allow(unused_variables)]
            let result = from.transition(to);

            // Aggregate-level transition (via Feature)
            let mut feature = Feature::new("fuzz-slug", "Fuzz Feature", [0u8; 32], None);
            // Set the starting state by reassigning
            feature.state = from;
            #[allow(unused_variables)]
            let feat_result = feature.transition(to);
        }

        // ---- FeatureState Display round-trip: to_string -> from_str ----
        if let Ok(state) = FeatureState::from_str(s) {
            let rendered = state.to_string();
            #[allow(unused_variables)]
            if let Ok(roundtripped) = FeatureState::from_str(&rendered) {
                // Round-trip: from_str -> Display -> from_str succeeded.
            }
        }
    }
});
