//! Tests for `clap_ext::error` (4 tests).

use clap_ext::error::CliError;

#[test]
fn from_io_error() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
    let e: CliError = io.into();
    assert!(matches!(e, CliError::Io(_)));
    assert!(e.to_string().contains("nope"));
}

#[test]
fn from_str_and_string() {
    let e1: CliError = "bad input".into();
    let e2: CliError = String::from("bad input").into();
    assert!(matches!(e1, CliError::Other(_)));
    assert!(matches!(e2, CliError::Other(_)));
    assert!(e1.to_string().contains("bad input"));
    assert!(e2.to_string().contains("bad input"));
}

#[test]
fn from_anyhow_error() {
    let a = anyhow::anyhow!("upstream failure");
    let e: CliError = a.into();
    assert!(matches!(e, CliError::Other(_)));
    assert!(e.to_string().contains("upstream failure"));
}

#[test]
fn display_formatting_per_variant() {
    let e1 = CliError::Config("bad yaml".into());
    let e2 = CliError::Parse("syntax".into());
    let e3 = CliError::Validation("schema".into());
    let e4 = CliError::NotFound("file".into());
    let e5 = CliError::PermissionDenied("/etc/shadow".into());
    let e6 = CliError::Network("timeout".into());
    assert_eq!(e1.to_string(), "Config error: bad yaml");
    assert_eq!(e2.to_string(), "Parse error: syntax");
    assert_eq!(e3.to_string(), "Validation error: schema");
    assert_eq!(e4.to_string(), "Not found: file");
    assert_eq!(e5.to_string(), "Permission denied: /etc/shadow");
    assert_eq!(e6.to_string(), "Network error: timeout");
}
