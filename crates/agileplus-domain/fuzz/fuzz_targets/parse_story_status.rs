// StoryStatus: Kanban work-state machine. No Display impl exists yet,
// but FromStr covers the input side of any regression.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::story::StoryStatus;

fuzz_target!(|data: &[u8]| {
    let s = std::str::from_utf8(data).unwrap_or("");
    let _ = StoryStatus::from_str(s);
});
