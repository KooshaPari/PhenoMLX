//! Shared arg structs that 60+ CLIs re-implement.

use clap::{Args, ValueEnum};
use std::path::PathBuf;

/// `-c, --config <path>` arg, used by all 60+ CLIs.
#[derive(Debug, Args, Clone)]
pub struct ConfigArg {
    /// Path to the config file (YAML/TOML/JSON)
    #[arg(short, long, env = "PHENOTYPE_CONFIG")]
    pub config: Option<PathBuf>,
}

/// `-v, --verbose` and `-q, --quiet` flags, mutually exclusive.
///
/// Use as `#[command(flatten)] verbosity: Verbosity` on the top-level CLI struct.
#[derive(Debug, Args, Clone, Copy, Default)]
#[group(multiple = false)]
pub struct Verbosity {
    /// Increase log verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress non-error output
    #[arg(short, long)]
    pub quiet: bool,
}

impl Verbosity {
    /// Convert to a tracing-subscriber EnvFilter directive.
    ///
    /// - `--quiet`         → ERROR
    /// - (default)         → INFO
    /// - `-v`              → DEBUG
    /// - `-vv` (or more)   → TRACE
    pub fn to_filter(&self) -> tracing_subscriber::filter::LevelFilter {
        match (self.verbose, self.quiet) {
            (_, true) => tracing_subscriber::filter::LevelFilter::ERROR,
            (0, false) => tracing_subscriber::filter::LevelFilter::INFO,
            (1, false) => tracing_subscriber::filter::LevelFilter::DEBUG,
            _ => tracing_subscriber::filter::LevelFilter::TRACE,
        }
    }
}

/// Output format flag: human, json, yaml.
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Yaml,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Human => write!(f, "human"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Yaml => write!(f, "yaml"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_default_maps_to_info() {
        let v = Verbosity::default();
        assert_eq!(v.to_filter(), tracing_subscriber::filter::LevelFilter::INFO);
    }

    #[test]
    fn verbosity_quiet_overrides_verbose() {
        // `--quiet` must win over any `-v` count, otherwise the user
        // would have to remember to drop `-v` when adding `-q`.
        let v = Verbosity {
            verbose: 3,
            quiet: true,
        };
        assert_eq!(
            v.to_filter(),
            tracing_subscriber::filter::LevelFilter::ERROR
        );
    }

    #[test]
    fn verbosity_count_maps_to_filter_levels() {
        let cases = [
            (0u8, tracing_subscriber::filter::LevelFilter::INFO),
            (1u8, tracing_subscriber::filter::LevelFilter::DEBUG),
            (2u8, tracing_subscriber::filter::LevelFilter::TRACE),
            (10u8, tracing_subscriber::filter::LevelFilter::TRACE),
        ];
        for (count, expected) in cases {
            let v = Verbosity {
                verbose: count,
                quiet: false,
            };
            assert_eq!(
                v.to_filter(),
                expected,
                "verbose={count} should map to {expected:?}"
            );
        }
    }

    #[test]
    fn output_format_display_matches_variant() {
        // `OutputFormat` is a clap `ValueEnum`, so Display is the
        // canonical user-facing name. The strings here are part of the
        // public contract — 60+ CLIs echo them to stderr and into
        // config files — so the mapping must be locked in by test.
        assert_eq!(format!("{}", OutputFormat::Human), "human");
        assert_eq!(format!("{}", OutputFormat::Json), "json");
        assert_eq!(format!("{}", OutputFormat::Yaml), "yaml");
    }

    #[test]
    fn output_format_default_is_human() {
        assert_eq!(OutputFormat::default(), OutputFormat::Human);
    }
}
