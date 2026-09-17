# Changelog

All notable changes to `clap-ext` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `common_subcommands::InitCmd::validate_template()` — defense-in-depth
  helper that rejects templates containing path separators (`/`, `\`)
  or `..` traversal segments. Use it when wiring `InitCmd` to a future
  template loader. `init` itself does **not** read template files today;
  this is purely a hardening helper for future consumers.

### Changed

### Deprecated

### Removed

- `clap-ext-macros` proc-macro crate. The
  `#[clap_ext_common_subcommands]` attribute was a no-op passthrough that
  advertised codegen it did not perform and had **zero usages** in the
  workspace. The 3 common variants remain available as standalone structs
  (`InitCmd`, `ValidateCmd`, `VersionCmd`) and as the bundled
  `CommonCommands` enum in `common_subcommands`.

### Fixed

### Security

## [0.1.0] — 2026-06-12

### Added

- **Initial release** of `clap-ext` — shared Rust CLI extension library
  for the Phenotype org.
- `common_args` module:
  - `ConfigArg` — `-c, --config <path>` with `PHENOTYPE_CONFIG` env var
  - `Verbosity` — `-v, --verbose` (count) + `--quiet` (mutually exclusive),
    with `to_filter()` mapping to `tracing_subscriber::filter::LevelFilter`
  - `OutputFormat` — `human` / `json` / `yaml` clap `ValueEnum`
- `common_subcommands` module:
  - `InitCmd` — `init <path> [-f] [-t <template>]`
  - `ValidateCmd` — `validate <path> [--strict]`
  - `VersionCmd` — `version` (zero-arg)
  - `CommonCommands` — bundled enum of all 3
  - `add_common_subcommands(cmd)` — imperative builder helper
  - `CommonSubcommand` trait alias
- `error` module:
  - `CliError` — unified error enum (Io, Config, Parse, Validation,
    NotFound, PermissionDenied, Network, Other) with `From<...>` for
    `io::Error`, `anyhow::Error`, `&str`, `String`
  - `CliResult<T>` — `Result<T, CliError>` type alias
  - `exit_with(err)` — print to stderr and exit 1
- `logging` module:
  - `setup_tracing(filter)` — `tracing-subscriber` setup with RUST_LOG honor
  - `setup_tracing_from_count(verbose_count, quiet)` — convenience wrapper
- `prelude` module — one-line import for the common case
- `examples/basic` — runnable demo CLI
- 13 tests across 4 test files:
  - `tests/common_args.rs` (4 tests)
  - `tests/common_subcommands.rs` (3 tests)
  - `tests/error.rs` (4 tests)
  - `tests/integration.rs` (2 tests, exercises the basic example)
- GitHub Actions CI workflow (`.github/workflows/ci.yml`)
- README, LICENSE-MIT, LICENSE-APACHE, .gitignore, CHANGELOG

### Notes

- This is the v0.1.0 cut for the V9-T3-5 rollout wave. Adoption PRs
  in 5 sample repos are filed under
  `feat/clap-ext-adopt-2026-06-11` branches.
- MSRV is **1.82** (matches the org-wide `clippy.toml` MSRV policy).
[Unreleased]: https://github.com/KooshaPari/clap-ext/compare/HEAD
