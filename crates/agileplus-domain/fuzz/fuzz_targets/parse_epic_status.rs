// EpicStatus: parallel to StoryStatus. Fuzz FromStr to catch
// regressions in case-folding, whitespace handling, or panic-on-unknown.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::epic::EpicStatus;

fuzz_target!(|data: &[u8]| {
    let s = std::str::from_utf8(data).unwrap_or("");
    let _ = EpicStatus::from_str(s);
});
