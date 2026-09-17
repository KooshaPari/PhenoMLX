// Serde deserialization must never panic on malformed JSON for any
// public domain entity. We exercise the user object; failures usually
// come from String→enum mapping or panicking parsers.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<agileplus_domain::domain::user::User>(data);
});
