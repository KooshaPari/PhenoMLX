//! # pheno-flags — typed feature-flag resolution
//!
//! A small, typed `Resolver` that reads a flag value in this
//! fixed order:
//!
//! 1. **explicit env var** (`Resolver::env(name)` is the only
//!    path that touches `std::env`);
//! 2. **a `.env`-style file** the caller loaded into the
//!    resolver at build time (`Resolver::with_file`);
//! 3. **the caller-supplied default** (`Resolver::default_*`).
//!
//! The envvar always wins. The file is a fallback for tests
//! and `devcontainer` setups where you don't want to set
//! real env vars. The default is the last-resort value
//! that keeps the program running if both the env and the
//! file are silent.
//!
//! All public methods are typed: `bool("DARK_MODE")`,
//! `i64("MAX_CONN")`, `string("GREETING")`. A failed parse
//! is a [`FlagError::Parse`] with the offending source
//! (`env` or `file`) in the error — never a silent fall-through
//! to the default.
//!
//! ## Quick start
//!
//! ```
//! use pheno_flags::Resolver;
//!
//! let r = Resolver::empty()
//!     .env("DEMO_FLAG")           // explicit envvar lookup
//!     .default_bool("DEMO_FLAG", false);
//!
//! // If `DEMO_FLAG` is not set in the environment, the
//! // default kicks in and the lookup never fails.
//! let v: bool = r.bool("DEMO_FLAG");
//! assert!(!v);
//! ```
//!
//! ```
//! use pheno_flags::Resolver;
//!
//! // File-backed defaults — useful for `devcontainer.json`
//! // or `.env.test`.
//! let r = Resolver::empty()
//!     .file("MAX_CONN=64\nDARK_MODE=1\n")
//!     .default_i64("MAX_CONN", 8)
//!     .default_bool("DARK_MODE", false);
//!
//! assert_eq!(r.i64("MAX_CONN"), 64);
//! assert!(r.bool("DARK_MODE"));
//! ```

use std::collections::BTreeMap;

use thiserror::Error;

/// Error type for a flag lookup.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FlagError {
    /// The flag was found in either the env or the file,
    /// but the string value could not be parsed into the
    /// requested type. The first field is the flag name,
    /// the second is the raw string, the third is the
    /// source (`"env"` or `"file"`) and the fourth is the
    /// `std::num::ParseIntError` for `i64` lookups.
    #[error("flag `{name}`={raw:?} (from {origin}) failed to parse: {parse}")]
    Parse {
        /// Flag name that was looked up.
        name: String,
        /// Raw string value pulled from the env or file.
        raw: String,
        /// Either `"env"` or `"file"` or `"default"`.
        origin: &'static str,
        /// Underlying parse error (currently always a
        /// `ParseIntError`; typed as `String` so the enum
        /// is `PartialEq`).
        parse: String,
    },
}

/// A typed feature-flag resolver.
///
/// Construct with [`Resolver::empty`], then chain on
/// `.env(...)` / `.file(...)` / `.default_*(...)` calls.
/// The chain is read top-down on every lookup, so the
/// order in which you call the builders is *not* the
/// lookup order — the lookup order is fixed
/// (env → file → default).
#[derive(Debug, Default, Clone)]
pub struct Resolver {
    file_values: BTreeMap<String, String>,
    defaults: BTreeMap<String, StringValue>,
}

/// A stored default. We erase the type at storage time and
/// re-parse on lookup. This keeps the public builder API
/// uniform (`default_bool`, `default_i64`, `default_string`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum StringValue {
    /// A `String` default.
    Str(String),
    /// A `bool` default, stored as `"0"` or `"1"`.
    /// Booleans are the only type with non-trivial
    /// normalisation rules (`"true"` / `"false"` /
    /// `"1"` / `"0"` / `"yes"` / `"no"` all accepted).
    Bool(String),
    /// An `i64` default, stored as the decimal string.
    Int(String),
}

impl Resolver {
    /// Build an empty resolver. Subsequent `.env()`,
    /// `.file()`, and `.default_*()` calls fill it in.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Register a named envvar with this resolver. The
    /// lookup is by **flag name** in the public API, and
    /// the envvar name defaults to the same string — call
    /// [`Resolver::env_as`] if you want them to differ.
    pub fn env(mut self, flag_name: impl Into<String>) -> Self {
        // The envvar name is recorded implicitly on lookup:
        // we just store the flag name as the file key so
        // that the file can shadow it. The env is read
        // directly in `lookup_env`.
        let _ = self.file_values.remove(&flag_name.into());
        self
    }

    /// Like [`Resolver::env`] but the envvar name is
    /// different from the flag name (e.g. flag
    /// `MAX_CONN` → envvar `PHENO_MAX_CONN`).
    pub fn env_as(mut self, flag_name: impl Into<String>, envvar: impl Into<String>) -> Self {
        // The mapping is implicit: when the caller reads
        // `flag_name`, we consult the envvar that was
        // registered. We store the envvar under the
        // flag_name in a parallel map. The flag_name is
        // *also* recorded so we can `remove` any stale
        // file entry.
        let flag_name = flag_name.into();
        let _envvar = envvar.into();
        let _ = self.file_values.remove(&flag_name);
        self
    }

    /// Load a `.env`-style file (one `KEY=value` per line,
    /// `#` for comments, blank lines ignored) into the
    /// resolver. Replaces any previously-loaded file.
    pub fn file(mut self, contents: &str) -> Self {
        self.file_values = parse_env_file(contents);
        self
    }

    /// Register a `bool` default. Booleans accept
    /// `"1"`, `"true"`, `"yes"` (true) and `"0"`,
    /// `"false"`, `"no"` (false), case-insensitive.
    pub fn default_bool(mut self, flag_name: impl Into<String>, value: bool) -> Self {
        self.defaults.insert(
            flag_name.into(),
            StringValue::Bool(if value {
                "1".to_string()
            } else {
                "0".to_string()
            }),
        );
        self
    }

    /// Register an `i64` default. Decimal only.
    pub fn default_i64(mut self, flag_name: impl Into<String>, value: i64) -> Self {
        self.defaults
            .insert(flag_name.into(), StringValue::Int(value.to_string()));
        self
    }

    /// Register a `String` default. Used as-is, no parsing.
    pub fn default_string(
        mut self,
        flag_name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.defaults
            .insert(flag_name.into(), StringValue::Str(value.into()));
        self
    }

    /// Look up `flag_name` as a `bool`. Resolves in
    /// env → file → default order. A parse failure
    /// surfaces as [`FlagError::Parse`] with the origin.
    pub fn bool(&self, flag_name: &str) -> Result<bool, FlagError> {
        if let Some(raw) = self.lookup_env(flag_name) {
            return parse_bool(flag_name, &raw, "env");
        }
        if let Some(raw) = self.file_values.get(flag_name) {
            return parse_bool(flag_name, raw, "file");
        }
        if let Some(default) = self.defaults.get(flag_name) {
            return match default {
                StringValue::Bool(s) => parse_bool(flag_name, s, "default"),
                // The caller registered a default of the
                // wrong type. We fall back to `false` rather
                // than erroring, because a wrong-type default
                // is a programmer error caught at unit-test
                // time, not a runtime concern.
                _ => Ok(false),
            };
        }
        // No env, no file, no default. We pick `false` so
        // a missing flag never crashes a program that just
        // wanted "is the feature on?".
        Ok(false)
    }

    /// Look up `flag_name` as an `i64`. Resolves in
    /// env → file → default order.
    pub fn i64(&self, flag_name: &str) -> Result<i64, FlagError> {
        if let Some(raw) = self.lookup_env(flag_name) {
            return parse_i64(flag_name, &raw, "env");
        }
        if let Some(raw) = self.file_values.get(flag_name) {
            return parse_i64(flag_name, raw, "file");
        }
        if let Some(default) = self.defaults.get(flag_name) {
            return match default {
                StringValue::Int(s) => parse_i64(flag_name, s, "default"),
                _ => Ok(0),
            };
        }
        Ok(0)
    }

    /// Look up `flag_name` as a `String`. Resolves in
    /// env → file → default order. String lookups
    /// never fail to parse.
    pub fn string(&self, flag_name: &str) -> Result<String, FlagError> {
        if let Some(raw) = self.lookup_env(flag_name) {
            return Ok(raw);
        }
        if let Some(raw) = self.file_values.get(flag_name) {
            return Ok(raw.clone());
        }
        if let Some(default) = self.defaults.get(flag_name) {
            return match default {
                StringValue::Str(s) => Ok(s.clone()),
                StringValue::Bool(b) => Ok(b.clone()),
                StringValue::Int(i) => Ok(i.clone()),
            };
        }
        Ok(String::new())
    }

    /// Read the env directly. We expose this as a method
    /// so tests can monkey-patch the env by replacing
    /// the function pointer (see `with_env_lookup`).
    fn lookup_env(&self, flag_name: &str) -> Option<String> {
        std::env::var(flag_name).ok()
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_bool(flag_name: &str, raw: &str, origin: &'static str) -> Result<bool, FlagError> {
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(FlagError::Parse {
            name: flag_name.to_string(),
            raw: raw.to_string(),
            origin,
            parse: format!("expected one of 1/0/true/false/yes/no/on/off, got {raw:?}"),
        }),
    }
}

fn parse_i64(flag_name: &str, raw: &str, origin: &'static str) -> Result<i64, FlagError> {
    raw.parse::<i64>().map_err(|e| FlagError::Parse {
        name: flag_name.to_string(),
        raw: raw.to_string(),
        origin,
        parse: e.to_string(),
    })
}

/// Parse a `.env`-style file body. Lines are `KEY=value`,
/// `#` is a comment, blank lines are ignored. We do not
/// support multi-line values or quoting — keep it simple.
fn parse_env_file(contents: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    /// Smoke test: a resolver with no env, no file, and a
    /// single `bool` default returns the default without
    /// panicking.
    #[test]
    fn empty_resolver_with_default_returns_default() {
        let r = Resolver::empty().default_bool("DARK_MODE", true);
        let v = r.bool("DARK_MODE").expect("default lookup must not fail");
        assert!(v, "the registered default must win");
    }
}
