//! Error types for the AgilePlus governance system

use thiserror::Error;

/// Result type alias for governance operations
pub type Result<T> = std::result::Result<T, GovernanceError>;

/// Errors that can occur during governance operations
#[derive(Error, Debug)]
pub enum GovernanceError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Network error connecting to remote governance
    #[error("Network error: {0}")]
    Network(String),

    /// Policy violation
    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Invalid channel transition
    #[error("Invalid channel transition from {from} to {to}")]
    InvalidChannelTransition { from: String, to: String },

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Operation not allowed
    #[error("Operation not allowed: {0}")]
    NotAllowed(String),

    /// Sync error
    #[error("Sync error: {0}")]
    Sync(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Rubric catalog parse or validation error
    #[error("Rubric error: {0}")]
    Rubric(String),
}

impl GovernanceError {
    /// Check if this is a policy-related error
    pub fn is_policy_error(&self) -> bool {
        matches!(
            self,
            GovernanceError::PolicyViolation(_) | GovernanceError::NotAllowed(_)
        )
    }

    /// Check if this is a rate limit error
    pub fn is_rate_limit_error(&self) -> bool {
        matches!(self, GovernanceError::RateLimitExceeded(_))
    }

    /// Get HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            GovernanceError::Config(_) => 400,
            GovernanceError::Database(_) => 500,
            GovernanceError::Network(_) => 503,
            GovernanceError::PolicyViolation(_) => 403,
            GovernanceError::RateLimitExceeded(_) => 429,
            GovernanceError::Auth(_) => 401,
            GovernanceError::InvalidChannelTransition { .. } => 400,
            GovernanceError::NotFound(_) => 404,
            GovernanceError::NotAllowed(_) => 403,
            GovernanceError::Sync(_) => 500,
            GovernanceError::Internal(_) => 500,
            GovernanceError::Rubric(_) => 422,
        }
    }
}

impl From<rusqlite::Error> for GovernanceError {
    fn from(err: rusqlite::Error) -> Self {
        GovernanceError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for GovernanceError {
    fn from(err: serde_json::Error) -> Self {
        GovernanceError::Config(err.to_string())
    }
}

impl From<config::ConfigError> for GovernanceError {
    fn from(err: config::ConfigError) -> Self {
        GovernanceError::Config(err.to_string())
    }
}

impl From<tokio::sync::mpsc::error::SendError<std::sync::Arc<dyn std::any::Any + Send + Sync>>>
    for GovernanceError
{
    fn from(
        err: tokio::sync::mpsc::error::SendError<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    ) -> Self {
        GovernanceError::Internal(err.to_string())
    }
}

impl From<std::io::Error> for GovernanceError {
    fn from(err: std::io::Error) -> Self {
        GovernanceError::Internal(err.to_string())
    }
}

impl From<toml::de::Error> for GovernanceError {
    fn from(err: toml::de::Error) -> Self {
        GovernanceError::Config(err.to_string())
    }
}
