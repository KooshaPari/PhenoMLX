# Functional Requirements — clap-ext

**Status:** Phase 3 spec+test+traceability layer (e2e Block B)
**Scope:** `clap-ext` v0.1.0 — the `clap-ext` crate at `crates/clap-ext/`
**Audience:** Maintainers, adopters integrating `clap-ext` into their own CLIs

This document enumerates the functional requirements (FRs) of the
`clap-ext` shared CLI extension library. Each FR maps to public API
constructs in the crate and is verified by a Rust `#[test]` function
annotated with `/// FR-XXX: …` immediately above its declaration.

Test functions live in two places:

- `src/lib.rs` — `#[cfg(test)] mod tests` for cross-module
  smoke / prelude-re-export tests.
- `crates/clap-ext/tests/fr_*.rs` — one file per FR (or FR group)
  exercising the public surface via the documented public API.

See [`TRACEABILITY.md`](./TRACEABILITY.md) for the FR → test fn →
implementation mapping.

---

## FR-001 — Library SHALL provide reusable clap arg structs

**Description:** The library SHALL expose derive-friendly clap arg
structs for the day-one flags that almost every CLI re-implements:

- `ConfigArg` — a `-c/--config <PATH>` arg backed by the
  `PHENOTYPE_CONFIG` environment variable.
- `Verbosity` — a `-v/--verbose` count flag plus a mutually
  exclusive `-q/--quiet` boolean, exposed via `to_filter()` for
  mapping onto a `tracing_subscriber` filter.
- `OutputFormat` — a clap `ValueEnum` with the variants `Human`,
  `Json`, and `Yaml` (default `Human`).

**Public API touched:** `clap_ext::common_args::{ConfigArg,
Verbosity, OutputFormat}`, re-exported by `clap_ext::prelude::*`.

**Implementation:** `crates/clap-ext/src/common_args.rs:1-63`.

**Tests:** see `crates/clap-ext/tests/common_args.rs` and
`crates/clap-ext/tests/fr_001_common_args.rs`.

---

## FR-002 — Library SHALL provide reusable common subcommands

**Description:** The library SHALL expose three standard subcommand
arg structs (`InitCmd`, `ValidateCmd`, `VersionCmd`), each derive-
compatible with `clap::Args` / `clap::Subcommand`. A convenience
enum `CommonCommands` SHALL bundle the three into a single
`Subcommand` variant for derived CLIs, and the function
`add_common_subcommands(cmd)` SHALL register the three
imperatively on a `clap::Command`. A `CommonSubcommand` trait SHALL
be implemented for each struct to enable uniform match-arm
signatures in adopter code.

**Public API touched:** `clap_ext::common_subcommands::{InitCmd,
ValidateCmd, VersionCmd, CommonCommands, add_common_subcommands,
CommonSubcommand}`, re-exported by `clap_ext::prelude::*`.

**Implementation:** `crates/clap-ext/src/common_subcommands.rs:1-83`.

**Tests:** see `crates/clap-ext/tests/common_subcommands.rs` and
`crates/clap-ext/tests/fr_002_common_subcommands.rs`.

---

## FR-003 — Library SHALL provide a unified `CliError` type

**Description:** The library SHALL provide a `thiserror`-backed
`CliError` enum covering the categories of error commonly produced
by CLIs:

- `Io(std::io::Error)` — file / network I/O failures,
  with `#[from] std::io::Error`.
- `Config(String)` — configuration-file parse or validation errors.
- `Parse(String)` — argv / config parse errors.
- `Validation(String)` — domain validation errors.
- `NotFound(String)` — missing-resource errors.
- `PermissionDenied(String)` — access-control errors.
- `Network(String)` — network/HTTP errors.
- `Other(anyhow::Error)` — catch-all, with `#[from] anyhow::Error`.

`From<&str>` and `From<String>` SHALL be implemented to wrap the
input in `CliError::Other`, so that `?` propagation from string
errors works uniformly. `Display` SHALL render each variant with a
deterministic prefix (e.g. `Config error: …`).

A `CliResult<T>` type alias (`Result<T, CliError>`) and an
`exit_with(err)` helper that prints to stderr and exits with code
1 SHALL also be exposed.

**Public API touched:** `clap_ext::error::{CliError, CliResult,
exit_with}`.

**Implementation:** `crates/clap-ext/src/error.rs:1-56`.

**Tests:** see `crates/clap-ext/tests/error.rs` and
`crates/clap-ext/tests/fr_003_error.rs`.

---

## FR-004 — Library SHALL provide tracing-subscriber setup

**Description:** The library SHALL provide a `setup_tracing(filter)`
function that installs a global `tracing_subscriber::fmt` layer
honoring `RUST_LOG` when set, and falling back to the supplied
`tracing_subscriber::filter::LevelFilter` otherwise. The setup
SHALL be idempotent (it SHALL swallow `SetGlobalDefaultError` so
that repeated calls from tests and library code are safe).

A `setup_tracing_from_count(verbose_count, quiet)` convenience
function SHALL be provided to map the same semantics as
`Verbosity::to_filter()` (count → level, with `quiet` overriding
to `ERROR`).

**Public API touched:** `clap_ext::logging::{setup_tracing,
setup_tracing_from_count}`.

**Implementation:** `crates/clap-ext/src/logging.rs:1-37`.

**Tests:** see `crates/clap-ext/tests/fr_004_logging.rs`.

---

## FR-005 — Library SHALL provide a domain-agnostic `CliPort` trait and a `clap`-backed adapter

**Description:** The library SHALL expose a `CliPort` trait that
decouples consumer code from the concrete `clap` builder API. The
trait SHALL expose four methods:

- `parse(args: &[&str]) -> Result<ParsedInvocation, CliError>`
- `help() -> String`
- `version() -> &str`
- `name() -> &str`

The trait SHALL be `Send + Sync` so it can be wrapped in
`Arc<dyn CliPort>` for long-running processes (REPLs, daemons,
MCP servers).

The library SHALL provide `ClapBasedCli`, a `clap`-builder-backed
implementation of `CliPort` that supports:

- Constructor `ClapBasedCli::new(name, about, version)`.
- `.with_author(author)` to populate the `--author` field.
- `.with_subcommand(name, about, help)` to register app-specific
  subcommands whose trailing positional args are captured verbatim
  into `ParsedCli::Other::args`.
- A `build_command()` method that exposes the underlying
  `clap::Command` for further extension.
- A `ParsedInvocation` value exposing `globals: GlobalOptions`
  and `command: ParsedCli` (with `Init`, `Validate`, `Version`,
  and `Other` variants).

**Public API touched:** `clap_ext::clap_based_cli::{CliPort,
ClapBasedCli, GlobalOptions, ParsedCli, ParsedInvocation}`.

**Implementation:** `crates/clap-ext/src/clap_based_cli.rs:1-339`.

**Tests:** see `crates/clap-ext/tests/fr_005_cli_port.rs` and the
existing in-module tests at
`crates/clap-ext/src/clap_based_cli.rs:345-488`.

---

## Cross-cutting requirements

- **Prelude:** the library SHALL re-export every public type
  listed in FR-001..FR-005 from `clap_ext::prelude::*` for the
  common "one import" use-case. This is verified by the
  `prelude_smoke_test` in `src/lib.rs`.
- **MSRV:** the library SHALL compile on Rust 1.82+ (per
  workspace `rust-version`). This is verified by the CI matrix
  pinned in `Cargo.toml`.
- **No panics on bad argv:** `CliPort::parse` SHALL return
  `Err(CliError::Parse(_))` rather than panicking for malformed
  argv (e.g. missing required subcommand, missing required
  positional, mutually-exclusive flag conflict). This is verified
  by the negative tests in
  `crates/clap-ext/src/clap_based_cli.rs:399-444` and
  `crates/clap-ext/tests/fr_005_cli_port.rs`.

## Out of scope (Phase 3)

- New subcommands beyond `init` / `validate` / `version`.
- New error variants.
- A second `CliPort` adapter (e.g. `lexopt` or `pico-args`).
- Performance benchmarks.

These are tracked for future phases.
