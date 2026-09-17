//! Integration tests for `pheno-flags`.
//!
//! These tests exercise the three lookup paths (envvar,
//! file, default) and the error shape on a parse failure.
//! We use a process-mutex to serialise the envvar-touching
//! tests so they can't race when cargo runs them in parallel.

use pheno_flags::{FlagError, Resolver};
use std::io::Write;
use std::sync::Mutex;

// Tests in this file touch process-global state (the
// environment). We serialise them with a mutex so they
// can't race when cargo runs them in parallel.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// Unique key per test so the tests can never see each
// other's environment.  The set/unset must happen on the
// same `unsafe` boundary guarded by `ENV_LOCK`.
const K_ENV_WINS: &str = "PHENO_TEST_ENV_WINS";
const K_FILE_WINS: &str = "PHENO_TEST_FILE_WINS";
const K_DEFAULT_WINS: &str = "PHENO_TEST_DEFAULT_WINS";
const K_PARSE_FAIL: &str = "PHENO_TEST_PARSE_FAIL";

/// Set an env var. The caller MUST hold `ENV_LOCK` and
/// MUST also call `unset` at the end of the test.
fn set(k: &str, v: &str) {
    // Safety: ENV_LOCK is held.
    unsafe {
        std::env::set_var(k, v);
    }
}

fn unset(k: &str) {
    // Safety: ENV_LOCK is held.
    unsafe {
        std::env::remove_var(k);
    }
}

#[test]
fn envvar_wins_over_file_and_default() {
    let _g = ENV_LOCK.lock().unwrap();
    set(K_ENV_WINS, "true");

    let r = Resolver::empty()
        .env(K_ENV_WINS)
        .file(&format!("{K_ENV_WINS}=false\n"))
        .default_bool(K_ENV_WINS, false);

    assert!(r.bool(K_ENV_WINS).unwrap());

    unset(K_ENV_WINS);
}

#[test]
fn file_wins_over_default_when_env_is_unset() {
    let _g = ENV_LOCK.lock().unwrap();
    unset(K_FILE_WINS);

    let r = Resolver::empty()
        .env(K_FILE_WINS)
        .file(&format!("{K_FILE_WINS}=1\n"))
        .default_bool(K_FILE_WINS, false);

    assert!(
        r.bool(K_FILE_WINS).unwrap(),
        "file value `1` must win over the default"
    );
}

#[test]
fn default_is_used_when_env_and_file_are_silent() {
    let _g = ENV_LOCK.lock().unwrap();
    unset(K_DEFAULT_WINS);

    let r = Resolver::empty()
        .env(K_DEFAULT_WINS)
        .file("OTHER_KEY=ignored\n")
        .default_i64(K_DEFAULT_WINS, 42);

    assert_eq!(r.i64(K_DEFAULT_WINS).unwrap(), 42);
}

#[test]
fn parse_error_carries_origin_and_raw() {
    let _g = ENV_LOCK.lock().unwrap();
    set(K_PARSE_FAIL, "not-a-number");

    let r = Resolver::empty().env(K_PARSE_FAIL);

    let err = r.i64(K_PARSE_FAIL).expect_err("parse must fail");
    match err {
        FlagError::Parse {
            name, raw, origin, ..
        } => {
            assert_eq!(name, K_PARSE_FAIL);
            assert_eq!(raw, "not-a-number");
            assert_eq!(origin, "env");
        }
    }

    unset(K_PARSE_FAIL);
}

#[test]
fn bool_accepts_truthy_and_falsy_strings() {
    for (raw, expected) in [
        ("1", true),
        ("true", true),
        ("TRUE", true),
        ("yes", true),
        ("on", true),
        ("0", false),
        ("false", false),
        ("no", false),
        ("off", false),
    ] {
        let r = Resolver::empty().file(&format!("K={raw}\n"));
        assert_eq!(
            r.bool("K").unwrap(),
            expected,
            "input {raw:?} should parse to {expected}",
        );
    }
}

#[test]
fn file_parser_ignores_comments_and_blank_lines() {
    // Write a temp .env file with comments and blank
    // lines, then read it back and run it through the
    // parser. This is the full-grammar happy path.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".env");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# this is a comment").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "GREETING=hello").unwrap();
        writeln!(f, "  # indented comment").unwrap();
        writeln!(f, "MAX_CONN=64").unwrap();
    }
    let body = std::fs::read_to_string(&path).unwrap();
    let r = Resolver::empty()
        .file(&body)
        .default_string("GREETING", "fallback")
        .default_i64("MAX_CONN", 8);
    assert_eq!(r.string("GREETING").unwrap(), "hello");
    assert_eq!(r.i64("MAX_CONN").unwrap(), 64);
}
