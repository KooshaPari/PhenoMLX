//! `CliError`: shared error enum for 60+ CLIs.

use thiserror::Error;

/// Unified CLI error type for clap-ext adopters.
///
/// Implements `From<std::io::Error>`, `From<anyhow::Error>`,
/// `From<&str>`, and `From<String>` so any error can be `?`-propagated
/// into a `CliResult<T>`.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError::Other(anyhow::anyhow!(s.to_string()))
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::Other(anyhow::anyhow!(s))
    }
}

/// Convenience: exit the process with a formatted error message and code 1.
pub fn exit_with<E: std::fmt::Display>(err: E) -> ! {
    eprintln!("error: {err}");
    std::process::exit(1);
}

/// Standard `Result` alias for CLIs.
pub type CliResult<T> = Result<T, CliError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_routes_to_other_variant() {
        let err: CliError = "boom".into();
        match &err {
            CliError::Other(inner) => assert_eq!(inner.to_string(), "boom"),
            other => panic!("expected Other, got {other:?}"),
        }
        assert_eq!(err.to_string(), "boom");
    }

    #[test]
    fn from_string_preserves_payload() {
        let err: CliError = String::from("owned-payload").into();
        match &err {
            CliError::Other(inner) => assert_eq!(inner.to_string(), "owned-payload"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn from_io_error_uses_io_variant() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: CliError = io.into();
        assert!(matches!(err, CliError::Io(_)));
        assert!(err.to_string().contains("I/O error"));
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn from_anyhow_routes_to_other_variant() {
        let inner = anyhow::anyhow!("nested");
        let err: CliError = inner.into();
        assert!(matches!(err, CliError::Other(_)));
        assert!(err.to_string().contains("nested"));
    }

    #[test]
    fn question_mark_propagation_works_across_string_and_io() {
        // The whole point of the From impls is that `?` should just work
        // in any function returning CliResult. Lock that in.
        fn read_or_fail() -> CliResult<String> {
            let s: std::io::Result<String> = Err(std::io::Error::other("x"));
            let _ = s?;
            Ok(String::new())
        }
        let err = read_or_fail().unwrap_err();
        assert!(matches!(err, CliError::Io(_)));

        fn make_err() -> CliResult<()> {
            Err("nope")?
        }
        let err = make_err().unwrap_err();
        assert!(matches!(err, CliError::Other(_)));
    }
}
