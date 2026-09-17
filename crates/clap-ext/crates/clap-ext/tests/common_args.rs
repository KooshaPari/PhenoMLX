//! Tests for `clap_ext::common_args` (4 tests).

use clap::Parser;
use clap_ext::common_args::{ConfigArg, OutputFormat, Verbosity};

#[derive(Debug, Parser)]
struct Wrapper {
    #[command(flatten)]
    config: ConfigArg,
    #[command(flatten)]
    verbosity: Verbosity,
    #[arg(long, value_enum, default_value_t)]
    output: OutputFormat,
}

#[test]
fn config_arg_parses_path() {
    let w = Wrapper::parse_from(["t", "--config", "/tmp/cfg.yaml"]);
    assert_eq!(
        w.config.config.as_ref().map(|p| p.to_str().unwrap()),
        Some("/tmp/cfg.yaml")
    );
}

#[test]
fn config_arg_defaults_to_none() {
    let w = Wrapper::parse_from(["t"]);
    assert!(w.config.config.is_none());
}

#[test]
fn verbosity_to_filter_progression() {
    let v0 = Verbosity {
        verbose: 0,
        quiet: false,
    };
    let v1 = Verbosity {
        verbose: 1,
        quiet: false,
    };
    let v2 = Verbosity {
        verbose: 2,
        quiet: false,
    };
    let q = Verbosity {
        verbose: 0,
        quiet: true,
    };
    use tracing_subscriber::filter::LevelFilter;
    assert_eq!(v0.to_filter(), LevelFilter::INFO);
    assert_eq!(v1.to_filter(), LevelFilter::DEBUG);
    assert_eq!(v2.to_filter(), LevelFilter::TRACE);
    assert_eq!(q.to_filter(), LevelFilter::ERROR);
}

#[test]
fn output_format_default_and_display() {
    let f = OutputFormat::default();
    assert_eq!(f, OutputFormat::Human);
    assert_eq!(format!("{}", f), "human");
    assert_eq!(format!("{}", OutputFormat::Json), "json");
    assert_eq!(format!("{}", OutputFormat::Yaml), "yaml");
}
