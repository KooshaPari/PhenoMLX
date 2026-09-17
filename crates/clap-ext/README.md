<!-- work-state: active | [========8/10] Block B consolidation in progress -->

# clap-ext

[![CI](https://github.com/KooshaPari/clap-ext/actions/workflows/ci.yml/badge.svg)](https://github.com/KooshaPari/clap-ext/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Crates.io](https://img.shields.io/crates/v/clap-ext.svg)](https://crates.io/crates/clap-ext)

**Shared Rust CLI extension library** for [`clap`](https://docs.rs/clap)-based CLIs.

`clap-ext` consolidates the boilerplate that 60+ Rust CLIs in the Phenotype
org (and countless other Rust projects) re-implement on day one:

- **Config flags** (`--config`, `--verbose`, `--quiet`, `--output-format`)
- **Common subcommands** (`init`, `validate`, `version`)
- **Unified error type** (`CliError` enum with thiserror + anyhow integration)
- **Tracing setup** (`setup_tracing(verbosity)`)

## Quick start

```toml
# Cargo.toml
[dependencies]
clap-ext = "0.1"
clap = { version = "4.5", features = ["derive", "env"] }
anyhow = "1.0"
tracing = "0.1"
```

```rust
use clap::Parser;
use clap_ext::prelude::*;

#[derive(Debug, Parser)]
#[command(name = "mycli", version, about = "My CLI")]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity,

    #[command(flatten)]
    config: ConfigArg,

    #[arg(long, value_enum, default_value_t)]
    output: OutputFormat,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    Init(InitCmd),
    Validate(ValidateCmd),
    Version(VersionCmd),
    /// ... your app-specific subcommands
}

fn main() -> CliResult<()> {
    let cli = Cli::parse();
    setup_tracing(cli.verbosity.to_filter());

    match cli.cmd {
        Cmd::Init(c)     => { /* ... */ }
        Cmd::Validate(c) => { /* ... */ }
        Cmd::Version(_)  => println!("mycli v{}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
```

That's it — `--config/-c`, `--verbose/-v`, `--quiet/-q`, `--output`,
`PHENOTYPE_CONFIG` env var, `RUST_LOG` env var, and 3 standard subcommands
are all wired up.

## What's in the box

| Module | Types | What it does |
|--------|-------|--------------|
| `common_args` | `ConfigArg`, `Verbosity`, `OutputFormat` | Shared arg structs (flatten with `#[command(flatten)]`) |
| `common_subcommands` | `InitCmd`, `ValidateCmd`, `VersionCmd`, `CommonCommands`, `add_common_subcommands()` | Shared subcommand structs + imperative helper |
| `error` | `CliError`, `CliResult`, `exit_with()` | Unified error enum (Io, Config, Parse, Validation, NotFound, Network, ...) |
| `logging` | `setup_tracing()`, `setup_tracing_from_count()` | tracing-subscriber setup with RUST_LOG honoring |
| `prelude` | re-exports | One-line import for the common case |

## Verbosity levels

`Verbosity::to_filter()` maps `-v` / `-vv` / `-vvv` and `--quiet` to
`tracing_subscriber::filter::LevelFilter`:

| Flag | Filter |
|------|--------|
| `--quiet` | `ERROR` |
| (default) | `INFO` |
| `-v` | `DEBUG` |
| `-vv` or more | `TRACE` |

`RUST_LOG` env var is honored when set (overrides the filter).

## Output formats

`OutputFormat` is a clap `ValueEnum` with 3 variants:

- `human` (default) — pretty-printed, terminal-friendly
- `json` — machine-readable JSON
- `yaml` — machine-readable YAML

## Error type

`CliError` covers the common CLI error categories:

```rust
pub enum CliError {
    Io(std::io::Error),
    Config(String),
    Parse(String),
    Validation(String),
    NotFound(String),
    PermissionDenied(String),
    Network(String),
    Other(anyhow::Error),
}
```

Implementations of `From<...>` are provided for `io::Error`,
`anyhow::Error`, `&str`, and `String`, so any error can be `?`-propagated:

```rust
fn load_config(path: &Path) -> CliResult<Config> {
    let raw = std::fs::read_to_string(path)?; // io::Error → CliError::Io
    let cfg: Config = serde_yaml::from_str(&raw)?; // anyhow::Error → CliError::Other
    Ok(cfg)
}
```

## Subcommands

The 3 common subcommands (`init`, `validate`, `version`) are available
as standalone arg structs and as a `CommonCommands` enum:

```rust
#[derive(Subcommand)]
enum Cmd {
    // Bundled convenience enum:
    Common(CommonCommands),

    // Or hand-pick the ones you want:
    Init(InitCmd),
    Version(VersionCmd),

    // Plus your app-specific ones:
    Serve { port: u16 },
}
```

For imperative `clap::Command` builders, use
`add_common_subcommands(cmd)` to register all 3 at once.

## Examples

The `examples/basic` workspace member is a runnable demo:

```sh
$ cargo run -p basic -- --help
Example CLI using clap-ext

Usage: basic [OPTIONS] <COMMAND>

Commands:
  init      Initialize a new project
  validate  Validate a config
  version   Print version
  run       Run a custom command
  ...

Options:
  -v, --verbose...       Increase log verbosity
  -q, --quiet            Suppress non-error output
  -c, --config <CONFIG>  Path to the config file [env: PHENOTYPE_CONFIG=]
      --output <OUTPUT>  [default: human] [possible values: human, json, yaml]
  ...
```

## Adoption in the wild

`clap-ext` is being adopted across 60+ Rust CLIs in the Phenotype org,
starting with this rollout wave (June 2026):

- `kmobile` — `--config`/`--verbose` + `Init` subcommand
- `PhenoVCS` (worktree-manager) — tracing-subscriber setup
- `PhenoProc` — Args struct + subcommand boilerplate
- `Tokn` (tokenledger) — multi-subcommand CLI
- `HeliosCLI` (codex-rs) — Args struct

See [CHANGELOG.md](CHANGELOG.md) for the rollout timeline.

## MSRV

`clap-ext` is tested on **Rust 1.82+** (matches the org-wide
`clippy.toml` MSRV policy).

## License

Dual-licensed under either of:

- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)

at your option.

## Contributing

Issues and PRs welcome. Please run `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` before submitting.
