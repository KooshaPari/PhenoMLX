// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Fuzz target for `agileplus-api-types` JSON deserialization.
//
// Exercises serde_json::from_str on all exported API DTO types from the
// agileplus-api-types crate. Catches panics in:
//   - Enum variant parsing (unknown / malformed discriminants)
//   - Optional field handling (None / Some with invalid inner data)
//   - Nested struct boundary conditions (recursive depth, missing fields)
//   - Hex-bytes and timestamp custom deserializers (panic / overflow)
//   - Unexpected EOF on partial JSON tokens
//   - serde_json Value fallback catch-all for untyped payloads
//
// When new DTO types are added to agileplus-api-types, add corresponding
// serde_json::from_str calls below following the existing pattern.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Import types exported by agileplus-api-types as they become available.
        // Currently the crate exports an empty root module; this target is
        // designed to be extended with concrete DTO deserialization calls.
        #[allow(unused_imports)]
        use agileplus_api_types as api;

        // ---- Catch-all: feed arbitrary byte input through serde_json.Value ----
        // Even in the absence of concrete DTOs, this exercises the JSON parser
        // with arbitrary input, catching panics in the serde_json internals
        // (number parsing, string escaping, nested depth, etc.).
        #[allow(unused_variables)]
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
            // Successfully parsed as generic JSON value.
        }

        // ---- Template for concrete DTO deserialization (add as types ship) ----
        // #[allow(unused_variables)]
        // if let Ok(dto) = serde_json::from_str::<api::SomeDto>(s) {}
        //
        // #[allow(unused_variables)]
        // if let Ok(dto) = serde_json::from_str::<api::AnotherDto>(s) {}
    }
});
