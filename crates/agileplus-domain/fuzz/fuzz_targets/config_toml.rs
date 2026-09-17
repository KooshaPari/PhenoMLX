// TOML configuration deserialization: arbitrary input must not panic.
// Catches: invalid TOML stack out-of-range, regex overflow on patterns.

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let s = std::str::from_utf8(data).unwrap_or("");
    let _ = toml::from_str::<toml::Value>(s);
});
