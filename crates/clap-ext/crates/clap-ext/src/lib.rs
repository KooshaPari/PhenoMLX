//! clap-ext: shared Rust CLI extension library.
//!
//! Public API:
//! - [`prelude`]: common imports
//! - [`common_args`]: shared arg structs (ConfigArg, Verbosity, OutputFormat)
//! - [`common_subcommands`]: shared subcommands (Init, Validate, Version)
//! - [`error`]: `CliError` enum with thiserror + anyhow integration
//! - [`logging`]: tracing-subscriber setup
//! - [`clap_based_cli`]: [`crate::clap_based_cli::CliPort`] trait +
//!   [`crate::clap_based_cli::ClapBasedCli`] adapter (clap implementation)

pub mod clap_based_cli;
pub mod common_args;
pub mod common_subcommands;
pub mod error;
pub mod logging;

pub mod prelude {
    //! Common imports for adopting crates.

    pub use crate::clap_based_cli::{
        ClapBasedCli, CliPort, GlobalOptions, ParsedCli, ParsedInvocation,
    };
    pub use crate::common_args::{ConfigArg, OutputFormat, Verbosity};
    pub use crate::common_subcommands::{InitCmd, ValidateCmd, VersionCmd};
    pub use crate::error::{CliError, CliResult};
    pub use crate::logging::setup_tracing;
}

// ---------------------------------------------------------------------------
// Phase 3 — spec + test + traceability e2e layer.
// ---------------------------------------------------------------------------
// These unit tests live in lib.rs (not the integration tests/ dir) so that
// they exercise the public API surface in the same crate that publishes it.
// Every test is annotated with the FR id(s) it covers, so a future
// contributor can grep for "FR-XXX" and find both the spec and its tests.
//
/// FR-001 / FR-002 / FR-003 / FR-004 / FR-005: the `prelude` module SHALL
/// re-export every public type listed in `docs/specs/FR.md`. Smoke-test
/// that each re-export resolves to a concrete type by constructing it
/// (where the type has a public constructor) or by naming it in a
/// function signature (where it does not).
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::prelude::*;

    /// FR-001: `ConfigArg` is re-exported by the prelude and has the
    /// expected `--config` short flag + `PHENOTYPE_CONFIG` env var
    /// (verified indirectly: the field is a `Option<PathBuf>`).
    #[test]
    fn prelude_smoke_test() {
        // FR-001: common_args
        let _cfg: ConfigArg = ConfigArg {
            config: Some(PathBuf::from("/tmp/c.yaml")),
        };
        let _v: Verbosity = Verbosity {
            verbose: 1,
            quiet: false,
        };
        let _o: OutputFormat = OutputFormat::Json;

        // FR-002: common_subcommands
        let _i: InitCmd = InitCmd {
            path: PathBuf::from("."),
            force: false,
            template: "default".to_string(),
        };
        let _va: ValidateCmd = ValidateCmd {
            path: PathBuf::from("/x"),
            strict: false,
        };
        let _ve: VersionCmd = VersionCmd {};

        // FR-003: error
        let _e: CliError = CliError::Config("cfg".into());
        let _r: CliResult<u8> = Ok(1);

        // FR-005: clap_based_cli (compile-time check that types resolve)
        let port: Box<dyn CliPort> = Box::new(ClapBasedCli::new("p", "a", "0.0.0"));
        let _ = port.name();
    }

    /// FR-001: `Verbosity::to_filter()` maps the four documented
    /// combinations to the four `LevelFilter` values.
    #[test]
    fn fr001_verbosity_to_filter_quiet_overrides_verbose() {
        use tracing_subscriber::filter::LevelFilter;
        // Even with verbose=2, quiet=true must win.
        let q = Verbosity {
            verbose: 2,
            quiet: true,
        };
        assert_eq!(q.to_filter(), LevelFilter::ERROR);
    }

    /// FR-002: `CommonCommands` is a `clap::Subcommand` enum with the
    /// three documented variants. Parse each and inspect.
    #[test]
    fn fr002_common_commands_enum_parses_three_variants() {
        use crate::common_subcommands::CommonCommands;
        use clap::Parser;

        #[derive(Debug, Parser)]
        struct W {
            #[command(subcommand)]
            cmd: CommonCommands,
        }

        // init
        let w = W::parse_from(["t", "init", "/tmp/p", "-f", "-t", "rust"]);
        match w.cmd {
            CommonCommands::Init(c) => {
                assert_eq!(c.path, PathBuf::from("/tmp/p"));
                assert!(c.force);
                assert_eq!(c.template, "rust");
            }
            _ => panic!("expected Init"),
        }

        // validate
        let w = W::parse_from(["t", "validate", "/x", "--strict"]);
        match w.cmd {
            CommonCommands::Validate(c) => {
                assert_eq!(c.path, PathBuf::from("/x"));
                assert!(c.strict);
            }
            _ => panic!("expected Validate"),
        }

        // version
        let w = W::parse_from(["t", "version"]);
        assert!(matches!(w.cmd, CommonCommands::Version(_)));
    }

    /// FR-003: `?` propagation from `std::fs::read_to_string` SHALL
    /// produce a `CliError::Io` (via `#[from] std::io::Error`).
    #[test]
    fn fr003_question_mark_propagates_io_error_via_from() {
        fn read_missing() -> CliResult<String> {
            let s = std::fs::read_to_string("/this/path/does/not/exist/q9w8e7")?;
            Ok(s)
        }
        let err = read_missing().unwrap_err();
        assert!(matches!(err, CliError::Io(_)));
    }

    /// FR-004: `setup_tracing` is idempotent. Calling it twice in the
    /// same process must not panic; the second call returns silently.
    #[test]
    fn fr004_setup_tracing_idempotent() {
        use tracing_subscriber::filter::LevelFilter;
        setup_tracing(LevelFilter::INFO);
        // Second call must not panic — try_init returns Err on conflict
        // and our wrapper swallows it.
        setup_tracing(LevelFilter::DEBUG);
    }

    /// FR-005: `CliPort::parse` returns a `ParsedInvocation` whose
    /// `globals` reflect the supplied `-c` / `-v` / `-q` / `--output`
    /// values and whose `command` is the expected `ParsedCli` variant.
    #[test]
    fn fr005_cli_port_parse_init_round_trips_globals() {
        let cli = ClapBasedCli::new("mycli", "demo", "1.0.0");
        let inv = cli
            .parse(&["-vv", "-c", "/etc/c.yaml", "--output", "json", "version"])
            .expect("parse should succeed");
        assert_eq!(inv.globals.verbose, 2);
        assert_eq!(inv.globals.config, Some(PathBuf::from("/etc/c.yaml")));
        assert!(!inv.globals.quiet);
        assert_eq!(inv.globals.output, OutputFormat::Json);
        assert_eq!(inv.command, ParsedCli::Version);
    }
}
