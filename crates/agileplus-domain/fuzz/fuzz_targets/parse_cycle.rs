// CycleState: 8-state machine driving sprint lifecycle. Fuzz
// FromStr to ensure arbitrary input never panics.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::cycle::CycleState;

fuzz_target!(|data: &[u8]| {
    let s = std::str::from_utf8(data).unwrap_or("");
    let _ = CycleState::from_str(s);
});
