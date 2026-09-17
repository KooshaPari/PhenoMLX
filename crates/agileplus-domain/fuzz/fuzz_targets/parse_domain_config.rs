// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Fuzz target for `agileplus-domain` TOML / JSON config parsing.
//
// Exercises deserialization of `AppConfig` from arbitrary byte sequences
// interpreted as TOML and JSON. Catches panics in:
//   - toml::from_str (serde deserialization of all nested config structs)
//   - serde_json::from_str (JSON-based config loading paths)
//   - hex_bytes deserializer (via ApiKey key_hash field during full-config parse)
//   - Default field initialisers for missing / unknown keys
//
// This target does not call AppConfig::validate() because it is pub(crate);
// the fuzzer catches panics and assertion failures at the deserialisation
// layer alone.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // ---- TOML: Exercise toml::from_str<AppConfig> ---------------------------
    if let Ok(s) = std::str::from_utf8(data) {
        // Fuzz the primary TOML-based config loading path.
        // AppConfig and all sub-configs derive Deserialize + have #[serde(default)]
        // on every field, so even an empty / garbage string may produce a valid
        // config. The fuzzer exercises edge cases in:
        //   - toml_edit / toml deserialisation internals
        //   - serde field defaulting
        //   - PathBuf, u16, bool, Vec<String>, Option<String> parsing
        #[allow(unused_variables)]
        if let Ok(config) = toml::from_str::<agileplus_domain::config::AppConfig>(s) {
            // Successfully deserialized a configuration from fuzzed TOML input.
            // validate() is pub(crate) and cannot be called from here, but the
            // deserializer already verified structural integrity.
        }

        // ---- JSON: Exercise serde_json deserialization of AppConfig ---------
        // The config is also deserializable from JSON; this exercises the JSON
        // parser with the same serde-derived Deserialize impl.
        #[allow(unused_variables)]
        if let Ok(config) = serde_json::from_str::<agileplus_domain::config::AppConfig>(s) {
            // Successfully deserialized from fuzzed JSON.
        }
    }
});
