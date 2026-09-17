// SPDX-License-Identifier: MIT OR Apache-2.0
//! Error type for `phenotype-dep-guard`.

use thiserror::Error;

/// Crate-wide error type.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error (manifest read, cache write, etc.).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse error.
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),

    /// JSON parse error.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP error.
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// OSV.dev returned a non-2xx status.
    #[error("osv: status {status}: {body}")]
    OsvStatus { status: u16, body: String },

    /// Manifest missing or malformed.
    #[error("manifest: {0}")]
    Manifest(String),

    /// Other.
    #[error("{0}")]
    Other(String),
}

/// Crate-wide Result alias.
pub type Result<T> = std::result::Result<T, Error>;
