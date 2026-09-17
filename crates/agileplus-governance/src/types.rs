//! Core types for the AgilePlus governance system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an audit event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEventId(pub String);

/// Unique identifier for a policy check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyCheckId(pub String);

/// Connection status to remote governance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    /// Connected to remote governance
    Connected,
    /// Disconnected, using local-only mode
    Disconnected,
    /// Error connecting
    Error,
    /// Governance disabled
    Disabled,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disabled
    }
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionStatus::Connected => write!(f, "connected"),
            ConnectionStatus::Disconnected => write!(f, "disconnected"),
            ConnectionStatus::Error => write!(f, "error"),
            ConnectionStatus::Disabled => write!(f, "disabled"),
        }
    }
}

/// Governance action categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    /// Release management actions
    Release,
    /// Repository operations
    Repository,
    /// Policy changes
    Policy,
    /// Audit operations
    Audit,
    /// Configuration changes
    Config,
    /// General operations
    General,
}

impl Default for ActionCategory {
    fn default() -> Self {
        Self::General
    }
}

impl std::str::FromStr for ActionCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "release" => Ok(Self::Release),
            "repository" => Ok(Self::Repository),
            "policy" => Ok(Self::Policy),
            "audit" => Ok(Self::Audit),
            "config" => Ok(Self::Config),
            "general" => Ok(Self::General),
            _ => Err(format!("Unknown action category: {}", s)),
        }
    }
}

/// Result of an operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationResult {
    /// Operation succeeded
    Success,
    /// Operation failed
    Failure,
    /// Operation partially succeeded
    PartialSuccess,
}

impl std::fmt::Display for OperationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationResult::Success => write!(f, "success"),
            OperationResult::Failure => write!(f, "failure"),
            OperationResult::PartialSuccess => write!(f, "partial_success"),
        }
    }
}

impl Default for OperationResult {
    fn default() -> Self {
        Self::Success
    }
}

impl std::str::FromStr for OperationResult {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "success" => Ok(OperationResult::Success),
            "failure" => Ok(OperationResult::Failure),
            "partial_success" | "partialsuccess" => Ok(OperationResult::PartialSuccess),
            _ => Err(format!("Unknown result: {}", s)),
        }
    }
}

/// Log level for audit entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" | "err" => Ok(LogLevel::Error),
            _ => Err(format!("Unknown log level: {}", s)),
        }
    }
}

/// Authentication method for remote governance
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMethod {
    ApiKey,
    BearerToken,
    None,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::ApiKey
    }
}

/// Governance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStats {
    /// Total audit events
    pub total: u64,
    /// Events today
    pub today: u64,
    /// Failed operations
    pub errors: u64,
    /// Events by level
    pub by_level: std::collections::HashMap<String, u64>,
    /// Top actions
    pub top_actions: Vec<TopAction>,
}

impl Default for GovernanceStats {
    fn default() -> Self {
        Self {
            total: 0,
            today: 0,
            errors: 0,
            by_level: std::collections::HashMap::new(),
            top_actions: Vec::new(),
        }
    }
}

/// Top action statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopAction {
    pub action: String,
    pub count: u64,
}

/// Governance status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStatus {
    /// Whether governance is initialized
    pub initialized: bool,
    /// Connection status
    pub connection_status: ConnectionStatus,
    /// Remote governance enabled
    pub remote_enabled: bool,
    /// Local governance enabled
    pub local_enabled: bool,
    /// Sync to remote enabled
    pub sync_enabled: bool,
    /// Last sync timestamp
    pub last_sync: Option<DateTime<Utc>>,
    /// Pending operations for sync
    pub pending_operations: u64,
    /// Configuration summary
    pub config: GovernanceStatusConfig,
    /// Statistics
    pub stats: GovernanceStats,
}

impl Default for GovernanceStatus {
    fn default() -> Self {
        Self {
            initialized: false,
            connection_status: ConnectionStatus::Disabled,
            remote_enabled: false,
            local_enabled: false,
            sync_enabled: false,
            last_sync: None,
            pending_operations: 0,
            config: GovernanceStatusConfig::default(),
            stats: GovernanceStats::default(),
        }
    }
}

/// Configuration summary for status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStatusConfig {
    pub governance_url: String,
    pub auth_method: AuthMethod,
    pub policy_enabled: bool,
    pub rate_limit_enabled: bool,
}

impl Default for GovernanceStatusConfig {
    fn default() -> Self {
        Self {
            governance_url: String::new(),
            auth_method: AuthMethod::ApiKey,
            policy_enabled: true,
            rate_limit_enabled: false,
        }
    }
}

impl AuditEventId {
    /// Generate a new audit event ID
    pub fn new() -> Self {
        Self(format!("evt_{}", Uuid::new_v4()))
    }
}

impl Default for AuditEventId {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyCheckId {
    /// Generate a new policy check ID
    pub fn new() -> Self {
        Self(format!("chk_{}", Uuid::new_v4()))
    }
}

impl Default for PolicyCheckId {
    fn default() -> Self {
        Self::new()
    }
}
