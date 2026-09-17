//! Port trait and `clap`-backed adapter for CLI invocations.
//!
//! This module introduces the [`CliPort`] abstraction: a domain-agnostic
//! interface for parsing a command-line invocation into a structured
//! [`ParsedInvocation`]. The port is intentionally small — it knows about
//! the common global flags and the three common subcommands that
//! `clap-ext` standardizes, but says nothing about `clap` itself.
//!
//! [`ClapBasedCli`] is the canonical adapter: it implements [`CliPort`]
//! on top of the `clap` builder API. Other adapters (e.g. a `pico-args`
//! or `lexopt` implementation, or an in-memory mock for tests) can be
//! slotted in by implementing [`CliPort`] against a different backend.
//!
//! ## Adding app-specific subcommands
//!
//! Use [`ClapBasedCli::with_subcommand`] to register an app-specific
//! subcommand. The adapter does not interpret the subcommand's trailing
//! positional args; it captures them verbatim into
//! [`ParsedCli::Other::args`] so the application can dispatch on them.
//!
//! ## Example
//!
//! ```
//! use clap_ext::clap_based_cli::{CliPort, ClapBasedCli};
//!
//! let cli = ClapBasedCli::new("mycli", "demo", "1.2.3")
//!     .with_subcommand("serve", "Start the server", "Server args");
//!
//! let inv = cli.parse(&["serve", "8080"]).unwrap();
//! assert_eq!(inv.globals.verbose, 0);
//! match inv.command {
//!     clap_ext::clap_based_cli::ParsedCli::Other { name, args } => {
//!         assert_eq!(name, "serve");
//!         assert_eq!(args, vec!["8080".to_string()]);
//!     }
//!     _ => panic!("expected Other"),
//! }
//! ```

use std::path::PathBuf;

use crate::common_args::OutputFormat;
use crate::common_subcommands::add_common_subcommands;
use crate::error::CliError;

// ---------------------------------------------------------------------------
// Port trait
// ---------------------------------------------------------------------------

/// Port for parsing a command-line invocation into a structured form.
///
/// `CliPort` is the seam between an application's domain logic and the
/// concrete argument parser. The trait surface is intentionally narrow:
///
/// - [`CliPort::parse`] turns a slice of argv (without `argv[0]`) into
///   a [`ParsedInvocation`].
/// - [`CliPort::help`] / [`CliPort::version`] / [`CliPort::name`] expose
///   the metadata an application needs to render its own `--help`
///   page, build shell-completion stubs, or report `--version`.
///
/// `Send + Sync` lets implementations live behind an `Arc<dyn CliPort>`
/// in long-running processes (REPLs, daemons, MCP servers).
pub trait CliPort: Send + Sync {
    /// Parse the given args (excluding the program name) into a
    /// [`ParsedInvocation`]. Returns [`CliError::Parse`] on failure.
    fn parse(&self, args: &[&str]) -> Result<ParsedInvocation, CliError>;

    /// Render the long help text for this CLI.
    fn help(&self) -> String;

    /// The version string reported by `--version` and the
    /// [`ParsedCli::Version`] subcommand.
    fn version(&self) -> &str;

    /// The binary name (used as `argv[0]` and in help banners).
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Global flags shared across all subcommands.
///
/// Mirrors the fields on [`crate::common_args::ConfigArg`] and
/// [`crate::common_args::Verbosity`] plus [`crate::common_args::OutputFormat`],
/// flattened into a single struct so the port is independent of the
/// derive macros that produce them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalOptions {
    /// `-c/--config` value (`PHENOTYPE_CONFIG` env var honored).
    pub config: Option<PathBuf>,
    /// `-v/--verbose` count (each `-v` adds 1).
    pub verbose: u8,
    /// `-q/--quiet` flag.
    pub quiet: bool,
    /// `--output` value (default [`OutputFormat::Human`]).
    pub output: OutputFormat,
}

/// The subcommand portion of a parsed invocation.
///
/// The three "common" subcommands from [`crate::common_subcommands`]
/// are first-class variants. App-specific subcommands registered via
/// [`ClapBasedCli::with_subcommand`] land in [`ParsedCli::Other`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCli {
    /// `init [path] [-f] [-t <template>]`
    Init {
        path: PathBuf,
        force: bool,
        template: String,
    },
    /// `validate <path> [--strict]`
    Validate { path: PathBuf, strict: bool },
    /// `version`
    Version,
    /// App-specific subcommand. `args` are the trailing positional
    /// args captured after the subcommand name; the application is
    /// responsible for further parsing.
    Other { name: String, args: Vec<String> },
}

/// A fully-parsed CLI invocation: global flags + a subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInvocation {
    pub globals: GlobalOptions,
    pub command: ParsedCli,
}

// ---------------------------------------------------------------------------
// clap adapter
// ---------------------------------------------------------------------------

/// [`CliPort`] implementation backed by the `clap` builder API.
///
/// Owns the metadata (name, about, version) and the list of custom
/// subcommands; the three common subcommands (`init`, `validate`,
/// `version`) are always available via [`crate::common_subcommands`].
pub struct ClapBasedCli {
    name: String,
    about: String,
    version: String,
    author: Option<String>,
    /// `(name, about, help)` tuples for app-specific subcommands.
    custom: Vec<(String, String, String)>,
}

impl ClapBasedCli {
    /// Create a new CLI with the given name, about, and version.
    pub fn new(
        name: impl Into<String>,
        about: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            about: about.into(),
            version: version.into(),
            author: None,
            custom: Vec::new(),
        }
    }

    /// Set the `--author` field shown in the long help.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Register an app-specific subcommand.
    ///
    /// The subcommand's parsed form will be delivered to the consumer
    /// as [`ParsedCli::Other`] with the captured trailing positional
    /// `args`. Custom subcommands are listed before the common ones in
    /// `--help` output so that an app's main commands appear first.
    pub fn with_subcommand(
        mut self,
        name: impl Into<String>,
        about: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        self.custom.push((name.into(), about.into(), help.into()));
        self
    }

    /// Borrow the metadata for inspection (e.g. logging, telemetry).
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn about(&self) -> &str {
        &self.about
    }

    /// Build the underlying [`clap::Command`]. Exposed for testing
    /// and for callers that need to extend the command further.
    pub fn build_command(&self) -> clap::Command {
        let mut cmd = clap::Command::new(self.name.clone())
            .about(self.about.clone())
            .version(self.version.clone())
            .subcommand_required(true)
            .arg_required_else_help(true)
            .arg(
                clap::Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("CONFIG")
                    .help("Path to the config file")
                    .env("PHENOTYPE_CONFIG")
                    .value_parser(clap::value_parser!(PathBuf)),
            )
            .arg(
                clap::Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .action(clap::ArgAction::Count)
                    .help("Increase log verbosity (-v, -vv, -vvv)"),
            )
            .arg(
                clap::Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("verbose")
                    .help("Suppress non-error output"),
            )
            .arg(
                clap::Arg::new("output")
                    .long("output")
                    .value_name("OUTPUT")
                    .value_parser(clap::value_parser!(OutputFormat))
                    .default_value("human")
                    .help("Output format (human, json, yaml)"),
            );

        if let Some(author) = &self.author {
            cmd = cmd.author(author.clone());
        }

        for (name, about, help) in &self.custom {
            cmd = cmd.subcommand(
                clap::Command::new(name.clone()).about(about.clone()).arg(
                    clap::Arg::new("args")
                        .num_args(0..)
                        .trailing_var_arg(true)
                        .allow_hyphen_values(true)
                        .help(help.clone()),
                ),
            );
        }

        add_common_subcommands(cmd)
    }
}

impl CliPort for ClapBasedCli {
    fn parse(&self, args: &[&str]) -> Result<ParsedInvocation, CliError> {
        // clap expects argv[0] to be the program name.
        let mut full: Vec<&str> = Vec::with_capacity(args.len() + 1);
        full.push(&self.name);
        full.extend(args.iter().copied());

        let matches = self
            .build_command()
            .try_get_matches_from(&full)
            .map_err(|e| CliError::Parse(e.to_string()))?;

        let globals = GlobalOptions {
            config: matches.get_one::<PathBuf>("config").cloned(),
            verbose: matches.get_count("verbose"),
            quiet: matches.get_flag("quiet"),
            output: *matches
                .get_one::<OutputFormat>("output")
                .unwrap_or(&OutputFormat::Human),
        };

        let command = match matches.subcommand() {
            Some(("init", m)) => ParsedCli::Init {
                path: m
                    .get_one::<PathBuf>("path")
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from(".")),
                force: m.get_flag("force"),
                template: m
                    .get_one::<String>("template")
                    .cloned()
                    .unwrap_or_else(|| "default".to_string()),
            },
            Some(("validate", m)) => {
                let path = m.get_one::<PathBuf>("path").cloned().ok_or_else(|| {
                    CliError::Parse("validate requires a <path> argument".to_string())
                })?;
                ParsedCli::Validate {
                    path,
                    strict: m.get_flag("strict"),
                }
            }
            Some(("version", _)) => ParsedCli::Version,
            Some((name, m)) => {
                let args: Vec<String> = m
                    .get_many::<String>("args")
                    .map(|v| v.cloned().collect())
                    .unwrap_or_default();
                ParsedCli::Other {
                    name: name.to_string(),
                    args,
                }
            }
            None => {
                return Err(CliError::Parse(
                    "a subcommand is required (try --help)".to_string(),
                ));
            }
        };

        Ok(ParsedInvocation { globals, command })
    }

    fn help(&self) -> String {
        // `render_help()` writes the help text to a `StyledStr` that
        // we can convert to a `String`. Disabling color keeps the
        // output deterministic for tests and embedding in docs.
        self.build_command()
            .color(clap::ColorChoice::Never)
            .render_help()
            .to_string()
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> ClapBasedCli {
        ClapBasedCli::new("test-cli", "test about", "0.1.0")
            .with_author("Phenotype")
            .with_subcommand("serve", "Start server", "Server args")
    }

    #[test]
    fn parses_init_with_defaults() {
        let inv = cli().parse(&["init"]).unwrap();
        assert_eq!(inv.globals.verbose, 0);
        assert!(!inv.globals.quiet);
        assert_eq!(inv.globals.output, OutputFormat::Human);
        assert_eq!(
            inv.command,
            ParsedCli::Init {
                path: PathBuf::from("."),
                force: false,
                template: "default".to_string(),
            }
        );
    }

    #[test]
    fn parses_init_with_overrides() {
        let inv = cli()
            .parse(&["init", "/tmp/proj", "-f", "-t", "rust"])
            .unwrap();
        assert_eq!(
            inv.command,
            ParsedCli::Init {
                path: PathBuf::from("/tmp/proj"),
                force: true,
                template: "rust".to_string(),
            }
        );
    }

    #[test]
    fn parses_validate_strict() {
        let inv = cli().parse(&["validate", "/etc/cfg", "--strict"]).unwrap();
        assert_eq!(
            inv.command,
            ParsedCli::Validate {
                path: PathBuf::from("/etc/cfg"),
                strict: true,
            }
        );
    }

    #[test]
    fn validate_requires_path() {
        let err = cli().parse(&["validate"]).unwrap_err();
        assert!(matches!(err, CliError::Parse(_)));
    }

    #[test]
    fn parses_version_subcommand() {
        let inv = cli().parse(&["version"]).unwrap();
        assert_eq!(inv.command, ParsedCli::Version);
    }

    #[test]
    fn parses_custom_subcommand() {
        let inv = cli().parse(&["serve", "8080", "--workers", "4"]).unwrap();
        match inv.command {
            ParsedCli::Other { name, args } => {
                assert_eq!(name, "serve");
                assert_eq!(args, vec!["8080", "--workers", "4"]);
            }
            _ => panic!("expected Other, got {:?}", inv.command),
        }
    }

    #[test]
    fn parses_global_flags() {
        let inv = cli()
            .parse(&["-vv", "-c", "/tmp/cfg.yaml", "--output", "json", "version"])
            .unwrap();
        assert_eq!(inv.globals.verbose, 2);
        assert_eq!(inv.globals.config, Some(PathBuf::from("/tmp/cfg.yaml")));
        assert_eq!(inv.globals.output, OutputFormat::Json);
        assert_eq!(inv.command, ParsedCli::Version);
    }

    #[test]
    fn quiet_conflicts_with_verbose() {
        // clap should reject `-v -q`; we surface that as CliError::Parse.
        let err = cli().parse(&["-v", "-q", "version"]).unwrap_err();
        assert!(matches!(err, CliError::Parse(_)));
    }

    #[test]
    fn missing_subcommand_errors() {
        let err = cli().parse(&[]).unwrap_err();
        assert!(matches!(err, CliError::Parse(_)));
    }

    #[test]
    fn help_contains_subcommands_and_flags() {
        let h = cli().help();
        assert!(h.contains("init"), "help should list init; got: {h}");
        assert!(
            h.contains("validate"),
            "help should list validate; got: {h}"
        );
        assert!(h.contains("version"), "help should list version; got: {h}");
        assert!(
            h.contains("serve"),
            "help should list custom subcommand; got: {h}"
        );
        assert!(
            h.contains("--config"),
            "help should describe --config; got: {h}"
        );
        assert!(
            h.contains("--verbose"),
            "help should describe --verbose; got: {h}"
        );
    }

    #[test]
    fn metadata_accessors() {
        let c = cli();
        assert_eq!(c.name(), "test-cli");
        assert_eq!(c.version(), "0.1.0");
        assert_eq!(c.about(), "test about");
        // Trait accessors agree with the inherent ones.
        let port: &dyn CliPort = &c;
        assert_eq!(port.name(), "test-cli");
        assert_eq!(port.version(), "0.1.0");
    }

    #[test]
    fn trait_is_object_safe() {
        // Compile-time check: we can put it behind an Arc<dyn CliPort>.
        let c: std::sync::Arc<dyn CliPort> = std::sync::Arc::new(cli());
        let inv = c.parse(&["version"]).unwrap();
        assert_eq!(inv.command, ParsedCli::Version);
    }
}
