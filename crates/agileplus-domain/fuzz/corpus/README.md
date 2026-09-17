// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Fuzz corpus directory.
//
// Place seed corpus files here to guide the fuzzer toward interesting code
// paths. Each sub-directory should be named after the fuzz target it seeds:
//
//   corpus/parse_domain_config/     — TOML & JSON config snippets
//   corpus/parse_requirement_id/    — identifiers, slugs, JSON payloads
//   corpus/api_types_deser/         — JSON DTO payloads (all API types)
//   corpus/sql_fragments/           — SQL fragments with comments, strings
//   corpus/state_machine/           — state names, transition pairs
//
// Seed files should be small (< 1 KiB) and exercise edge cases the fuzzer
// might not discover through random mutation alone.
//
// Run a target with corpus seeding:
//   cargo +nightly fuzz run <target> --fuzz-dir crates/agileplus-domain/fuzz \
//     crates/agileplus-domain/fuzz/corpus/<target>/
