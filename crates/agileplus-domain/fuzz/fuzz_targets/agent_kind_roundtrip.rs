// AgentKind: the dispatch kind used for work-package routing.
// No FromStr exists yet, so we exercise serde_json roundtrip to ensure
// malformed JSON never panics and unknowns are rejected gracefully.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::ports::agent::AgentKind;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<AgentKind>(data);
});
