//! Tests for `clap_ext::common_subcommands` (3 tests).

use clap::Parser;
use clap_ext::common_subcommands::{InitCmd, ValidateCmd, VersionCmd};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Wrapper {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    Init(InitCmd),
    Validate(ValidateCmd),
    Version(VersionCmd),
}

#[test]
fn init_cmd_default_path_and_template() {
    let w = Wrapper::parse_from(["t", "init"]);
    match w.cmd {
        Cmd::Init(c) => {
            assert_eq!(c.path, PathBuf::from("."));
            assert!(!c.force);
            assert_eq!(c.template, "default");
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn validate_cmd_strict_flag() {
    let w = Wrapper::parse_from(["t", "validate", "/tmp/x", "--strict"]);
    match w.cmd {
        Cmd::Validate(c) => {
            assert_eq!(c.path, PathBuf::from("/tmp/x"));
            assert!(c.strict);
        }
        _ => panic!("expected Validate"),
    }
}

#[test]
fn version_cmd_unit_struct() {
    let w = Wrapper::parse_from(["t", "version"]);
    match w.cmd {
        Cmd::Version(_) => {}
        _ => panic!("expected Version"),
    }
}
