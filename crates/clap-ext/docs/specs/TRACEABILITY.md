# Traceability Matrix — clap-ext (Phase 3)

This document maps every functional requirement in
[`FR.md`](./FR.md) to the Rust `#[test]` function(s) that
verify it and to the implementation site in the source tree.

**Conventions**

- Test functions are tagged with the FR(s) they verify in a
  `/// FR-XXX: <one-line description>` doc comment immediately
  above the `#[test]` attribute. This makes FRs greppable from
  the test source.
- The "Test" column lists the test function name and the file
  in which it lives. Format: `path:fn` (e.g.
  `crates/clap-ext/src/lib.rs:prelude_smoke_test`).
- The "Impl" column lists the implementation site by
  `file:startLine-endLine` (range) or `file:startLine` (single).
- Status legend: ✅ implemented + tested · ⚠ partial · ❌ missing.

---

## FR-001 — Reusable clap arg structs

| Test                                                       | Status |
|------------------------------------------------------------|--------|
| `crates/clap-ext/src/lib.rs:prelude_smoke_test`            | ✅     |
| `crates/clap-ext/tests/common_args.rs:config_arg_parses_path` | ✅  |
| `crates/clap-ext/tests/common_args.rs:config_arg_defaults_to_none` | ✅ |
| `crates/clap-ext/tests/common_args.rs:verbosity_to_filter_progression` | ✅ |
| `crates/clap-ext/tests/common_args.rs:output_format_default_and_display` | ✅ |
| `crates/clap-ext/tests/fr_001_common_args.rs:fr001_prelude_reexports_config_arg` | ✅ |
| `crates/clap-ext/tests/fr_001_common_args.rs:fr001_prelude_reexports_verbosity` | ✅ |
| `crates/clap-ext/tests/fr_001_common_args.rs:fr001_prelude_reexports_output_format` | ✅ |

**Implementation:** `crates/clap-ext/src/common_args.rs:1-63`
(re-exported from `crates/clap-ext/src/lib.rs:18-28`).

---

## FR-002 — Reusable common subcommands

| Test | Status |
|------|--------|
| `crates/clap-ext/tests/common_subcommands.rs:init_cmd_default_path_and_template` | ✅ |
| `crates/clap-ext/tests/common_subcommands.rs:validate_cmd_strict_flag` | ✅ |
| `crates/clap-ext/tests/common_subcommands.rs:version_cmd_unit_struct` | ✅ |
| `crates/clap-ext/tests/fr_002_common_subcommands.rs:fr002_common_commands_enum_has_three_variants` | ✅ |
| `crates/clap-ext/tests/fr_002_common_subcommands.rs:fr002_add_common_subcommands_registers_three` | ✅ |
| `crates/clap-ext/tests/fr_002_common_subcommands.rs:fr002_common_subcommand_trait_is_impl_for_all_three` | ✅ |
| `crates/clap-ext/tests/fr_002_common_subcommands.rs:fr002_prelude_reexports_init_validate_version` | ✅ |

**Implementation:** `crates/clap-ext/src/common_subcommands.rs:1-83`
(re-exported from `crates/clap-ext/src/lib.rs:25`).

---

## FR-003 — Unified `CliError`

| Test | Status |
|------|--------|
| `crates/clap-ext/tests/error.rs:from_io_error` | ✅ |
| `crates/clap-ext/tests/error.rs:from_str_and_string` | ✅ |
| `crates/clap-ext/tests/error.rs:from_anyhow_error` | ✅ |
| `crates/clap-ext/tests/error.rs:display_formatting_per_variant` | ✅ |
| `crates/clap-ext/tests/fr_003_error.rs:fr003_cli_result_is_result_alias` | ✅ |
| `crates/clap-ext/tests/fr_003_error.rs:fr003_question_mark_propagates_io_error` | ✅ |
| `crates/clap-ext/tests/fr_003_error.rs:fr003_prelude_reexports_cli_error_and_result` | ✅ |

**Implementation:** `crates/clap-ext/src/error.rs:1-56`
(re-exported from `crates/clap-ext/src/lib.rs:26`).

---

## FR-004 — Tracing-subscriber setup

| Test | Status |
|------|--------|
| `crates/clap-ext/tests/fr_004_logging.rs:fr004_setup_tracing_is_idempotent` | ✅ |
| `crates/clap-ext/tests/fr_004_logging.rs:fr004_setup_tracing_from_count_maps_levels` | ✅ |
| `crates/clap-ext/tests/fr_004_logging.rs:fr004_setup_tracing_accepts_filter` | ✅ |

**Implementation:** `crates/clap-ext/src/logging.rs:1-37`
(re-exported from `crates/clap-ext/src/lib.rs:27`).

---

## FR-005 — `CliPort` trait + `ClapBasedCli` adapter

| Test | Status |
|------|--------|
| `crates/clap-ext/src/clap_based_cli.rs:parses_init_with_defaults` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:parses_init_with_overrides` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:parses_validate_strict` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:validate_requires_path` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:parses_version_subcommand` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:parses_custom_subcommand` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:parses_global_flags` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:quiet_conflicts_with_verbose` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:missing_subcommand_errors` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:help_contains_subcommands_and_flags` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:metadata_accessors` | ✅ |
| `crates/clap-ext/src/clap_based_cli.rs:trait_is_object_safe` | ✅ |
| `crates/clap-ext/tests/fr_005_cli_port.rs:fr005_parse_init_through_port_trait` | ✅ |
| `crates/clap-ext/tests/fr_005_cli_port.rs:fr005_parse_other_preserves_trailing_args` | ✅ |
| `crates/clap-ext/tests/fr_005_cli_port.rs:fr005_global_options_default_human_output` | ✅ |
| `crates/clap-ext/tests/fr_005_cli_port.rs:fr005_arc_dyn_cli_port_works` | ✅ |
| `crates/clap-ext/tests/fr_005_cli_port.rs:fr005_prelude_reexports_port_types` | ✅ |

**Implementation:** `crates/clap-ext/src/clap_based_cli.rs:1-339`
(re-exported from `crates/clap-ext/src/lib.rs:21-23`).

---

## Cross-cutting checks

| Requirement | Test | Status |
|-------------|------|--------|
| Prelude re-exports every public type | `crates/clap-ext/src/lib.rs:prelude_smoke_test` | ✅ |
| No panics on bad argv | `crates/clap-ext/src/clap_based_cli.rs:validate_requires_path` | ✅ |
| No panics on bad argv | `crates/clap-ext/src/clap_based_cli.rs:quiet_conflicts_with_verbose` | ✅ |
| No panics on bad argv | `crates/clap-ext/src/clap_based_cli.rs:missing_subcommand_errors` | ✅ |

---

## Coverage summary

| FR | # Tests | Status |
|----|---------|--------|
| FR-001 | 8 | ✅ |
| FR-002 | 7 | ✅ |
| FR-003 | 7 | ✅ |
| FR-004 | 3 | ✅ |
| FR-005 | 17 | ✅ |
| **Total** | **42** | ✅ |

Every functional requirement in [`FR.md`](./FR.md) has at least
one test in the workspace test suite.
