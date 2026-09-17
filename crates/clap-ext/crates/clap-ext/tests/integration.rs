//! Integration tests for the basic example (2 tests).
//!
//! These spawn `cargo run -p basic` to exercise the example CLI
//! end-to-end (clap parsing + subcommand dispatch + tracing setup).
//! We use `cargo run` rather than the binary path because the
//! `examples/basic` package is a workspace member and is rebuilt on
//! every test run.

use std::path::PathBuf;
use std::process::Command;

fn cargo_run() -> Command {
    // manifest_dir = .../clap-ext/crates/clap-ext
    // workspace root = .../clap-ext
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&workspace_root)
        .arg("run")
        .arg("-q")
        .arg("-p")
        .arg("basic")
        .arg("--");
    cmd
}

#[test]
fn basic_help_renders() {
    let output = cargo_run()
        .arg("--help")
        .output()
        .expect("spawn cargo run -p basic -- --help");
    assert!(output.status.success(), "--help should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("basic"), "help must mention binary name");
    assert!(stdout.contains("--config"), "help must list --config");
    assert!(stdout.contains("--verbose"), "help must list --verbose");
}

#[test]
fn basic_verbose_flag_parses() {
    // --verbose should parse without error; subcommand is required so
    // we also pass `version` (the simplest zero-side-effect subcommand).
    let output = cargo_run()
        .arg("-vv")
        .arg("version")
        .output()
        .expect("spawn cargo run -p basic -- -vv version");
    assert!(
        output.status.success(),
        "-vv version should exit 0; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("basic v"),
        "version subcommand should print version; got: {stdout}"
    );
}
