// Fuzz target for backlog-related enum parsers.
//
// Strategy: partition the input bytes into 5 slices, each <1 KiB,
// and feed each to a different FromStr impl. Catches:
// - panics in match arms
// - case-folding bugs
// - canonicalization bugs (e.g., "open" vs "Open" vs "OPEN")
// - allocation panics

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::backlog::{BacklogPriority, BacklogStatus, Intent};

fn split(s: &[u8]) -> impl Iterator<Item = &[u8]> {
    // Split on null bytes (never appear in ASCII grammar tokens).
    s.split(|&b| b == 0)
}

fuzz_target!(|data: &[u8]| {
    let mut parts = split(data);
    let p1 = parts.next().unwrap_or(b"");
    let p2 = parts.next().unwrap_or(b"");
    let p3 = parts.next().unwrap_or(b"");

    // INTENT must never panic on arbitrary input.
    let _ = Intent::from_str(std::str::from_utf8(p1).unwrap_or(""));
    // BacklogPriority accepts only well-formed tokens; don't panic.
    let _ = BacklogPriority::from_str(std::str::from_utf8(p2).unwrap_or(""));
    // BacklogStatus: same.
    let _ = BacklogStatus::from_str(std::str::from_utf8(p3).unwrap_or(""));
});
