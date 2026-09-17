//! Audit logging for governance actions
//!
//! Provides complete audit trail of all governance operations,
//! with local SQLite storage and optional remote sync.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

use crate::error::{GovernanceError, Result};
use crate::types::*;

/// Audit event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Action performed
    pub action: String,
    /// Action category
    pub category: Option<ActionCategory>,
    /// Human-readable message
    pub message: Option<String>,
    /// User ID
    pub user_id: Option<String>,
    /// Client IP
    pub client_ip: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Request ID
    pub request_id: Option<String>,
    /// Resource type
    pub resource: Option<String>,
    /// Resource ID
    pub resource_id: Option<String>,
    /// HTTP method
    pub method: Option<String>,
    /// Endpoint
    pub endpoint: Option<String>,
    /// Request parameters
    pub parameters: Option<serde_json::Value>,
    /// Operation result
    pub result: OperationResult,
    /// Error code
    pub error_code: Option<String>,
    /// Error message
    pub error_message: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
    /// Stack trace (for errors)
    pub stack_trace: Option<String>,
    /// Sync timestamp (for remote sync)
    pub synced_at: Option<DateTime<Utc>>,
    /// Created at
    pub created_at: DateTime<Utc>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(action: impl Into<String>, level: LogLevel, result: OperationResult) -> Self {
        Self {
            id: AuditEventId::new().0,
            timestamp: Utc::now(),
            level,
            action: action.into(),
            category: None,
            message: None,
            user_id: None,
            client_ip: None,
            user_agent: None,
            session_id: None,
            request_id: None,
            resource: None,
            resource_id: None,
            method: None,
            endpoint: None,
            parameters: None,
            result,
            error_code: None,
            error_message: None,
            duration_ms: None,
            metadata: None,
            stack_trace: None,
            synced_at: None,
            created_at: Utc::now(),
        }
    }

    /// Create a success event
    pub fn success(action: impl Into<String>) -> Self {
        Self::new(action, LogLevel::Info, OperationResult::Success)
    }

    /// Create a warning event
    pub fn warn(action: impl Into<String>) -> Self {
        Self::new(action, LogLevel::Warn, OperationResult::Success)
    }

    /// Create an error event
    pub fn error(action: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(action, LogLevel::Error, OperationResult::Failure).with_message(message)
    }

    /// Set the message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set the action
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = action.into();
        self
    }

    /// Set the category
    pub fn with_category(mut self, category: ActionCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Set the user ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set the client IP
    pub fn with_client_ip(mut self, ip: impl Into<String>) -> Self {
        self.client_ip = Some(ip.into());
        self
    }

    /// Set the user agent
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Set the request context
    pub fn with_request(
        mut self,
        method: impl Into<String>,
        endpoint: impl Into<String>,
        request_id: Option<String>,
    ) -> Self {
        self.method = Some(method.into());
        self.endpoint = Some(endpoint.into());
        self.request_id = request_id.or_else(|| Some(format!("req_{}", Uuid::new_v4())));
        self
    }

    /// Set the resource
    pub fn with_resource(
        mut self,
        resource: impl Into<String>,
        resource_id: Option<String>,
    ) -> Self {
        self.resource = Some(resource.into());
        self.resource_id = resource_id;
        self
    }

    /// Set parameters
    pub fn with_parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = Some(params);
        self
    }

    /// Set duration
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set error details
    pub fn with_error(mut self, code: impl Into<String>, message: impl Into<String>) -> Self {
        self.error_code = Some(code.into());
        self.error_message = Some(message.into());
        self
    }

    /// Set the result
    pub fn with_result(mut self, result: OperationResult) -> Self {
        self.result = result;
        self
    }
}

/// Audit log query filter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    /// Filter by action
    pub action: Option<String>,
    /// Filter by category
    pub category: Option<ActionCategory>,
    /// Filter by level
    pub level: Option<LogLevel>,
    /// Filter by result
    pub result: Option<OperationResult>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by resource
    pub resource: Option<String>,
    /// Filter by start time
    pub start_time: Option<DateTime<Utc>>,
    /// Filter by end time
    pub end_time: Option<DateTime<Utc>>,
    /// Filter unsynced only
    pub unsynced_only: bool,
    /// Maximum results
    pub limit: u64,
    /// Offset for pagination
    pub offset: u64,
}

impl AuditFilter {
    /// Create a new filter with default limits
    pub fn new() -> Self {
        Self {
            limit: 100,
            offset: 0,
            ..Default::default()
        }
    }

    /// Filter by action
    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Filter by user
    pub fn user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Filter by time range
    pub fn time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    /// Get only unsynced events
    pub fn unsynced(mut self) -> Self {
        self.unsynced_only = true;
        self
    }

    /// Limit results
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }
}

/// Audit logger for local storage
pub struct AuditLogger {
    conn: Mutex<Connection>,
    retention_days: u32,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(db_path: impl AsRef<Path>, retention_days: u32) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let logger = Self {
            conn: Mutex::new(conn),
            retention_days,
        };
        logger.init_schema()?;
        Ok(logger)
    }

    /// Create in-memory logger for testing
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let logger = Self {
            conn: Mutex::new(conn),
            retention_days: 90,
        };
        logger.init_schema()?;
        Ok(logger)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                action TEXT NOT NULL,
                category TEXT,
                message TEXT,
                user_id TEXT,
                client_ip TEXT,
                user_agent TEXT,
                session_id TEXT,
                request_id TEXT,
                resource TEXT,
                resource_id TEXT,
                method TEXT,
                endpoint TEXT,
                parameters TEXT,
                result TEXT NOT NULL,
                error_code TEXT,
                error_message TEXT,
                duration_ms INTEGER,
                metadata TEXT,
                stack_trace TEXT,
                synced_at TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events(timestamp)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_events(action)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_synced ON audit_events(synced_at)",
            [],
        )?;

        Ok(())
    }

    /// Log an audit event
    pub fn log(&self, event: &AuditEvent) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;

        conn.execute(
            "INSERT INTO audit_events (
                id, timestamp, level, action, category, message, user_id,
                client_ip, user_agent, session_id, request_id, resource,
                resource_id, method, endpoint, parameters, result, error_code,
                error_message, duration_ms, metadata, stack_trace, synced_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                event.id,
                event.timestamp.to_rfc3339(),
                event.level.to_string(),
                event.action,
                event.category.map(|c| format!("{:?}", c)),
                event.message,
                event.user_id,
                event.client_ip,
                event.user_agent,
                event.session_id,
                event.request_id,
                event.resource,
                event.resource_id,
                event.method,
                event.endpoint,
                event.parameters.as_ref().map(|p| p.to_string()),
                event.result.to_string(),
                event.error_code,
                event.error_message,
                event.duration_ms.map(|d| d as i64),
                event.metadata.as_ref().map(|m| m.to_string()),
                event.stack_trace,
                event.synced_at.map(|s| s.to_rfc3339()),
                event.created_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Query audit events
    pub fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;

        let mut sql = String::from("SELECT * FROM audit_events WHERE 1=1");
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(ref action) = filter.action {
            sql.push_str(" AND action = ?");
            params_vec.push(action.clone());
        }

        if let Some(ref category) = filter.category {
            sql.push_str(" AND category = ?");
            params_vec.push(format!("{:?}", category));
        }

        if let Some(ref level) = filter.level {
            sql.push_str(" AND level = ?");
            params_vec.push(level.to_string());
        }

        if let Some(ref result) = filter.result {
            sql.push_str(" AND result = ?");
            params_vec.push(result.to_string());
        }

        if let Some(ref user_id) = filter.user_id {
            sql.push_str(" AND user_id = ?");
            params_vec.push(user_id.clone());
        }

        if let Some(ref resource) = filter.resource {
            sql.push_str(" AND resource = ?");
            params_vec.push(resource.clone());
        }

        if let Some(ref start) = filter.start_time {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(start.to_rfc3339());
        }

        if let Some(ref end) = filter.end_time {
            sql.push_str(" AND timestamp <= ?");
            params_vec.push(end.to_rfc3339());
        }

        if filter.unsynced_only {
            sql.push_str(" AND synced_at IS NULL");
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        params_vec.push(filter.limit.to_string());
        params_vec.push(filter.offset.to_string());

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(AuditEvent {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                level: row.get::<_, String>(2)?.parse().unwrap_or(LogLevel::Info),
                action: row.get(3)?,
                category: row
                    .get::<_, Option<String>>(4)?
                    .and_then(|c| c.parse().ok()),
                message: row.get(5)?,
                user_id: row.get(6)?,
                client_ip: row.get(7)?,
                user_agent: row.get(8)?,
                session_id: row.get(9)?,
                request_id: row.get(10)?,
                resource: row.get(11)?,
                resource_id: row.get(12)?,
                method: row.get(13)?,
                endpoint: row.get(14)?,
                parameters: row
                    .get::<_, Option<String>>(15)?
                    .and_then(|p| serde_json::from_str(&p).ok()),
                result: row
                    .get::<_, String>(16)?
                    .parse()
                    .unwrap_or(OperationResult::Success),
                error_code: row.get(17)?,
                error_message: row.get(18)?,
                duration_ms: row.get::<_, Option<i64>>(19)?.map(|d| d as u64),
                metadata: row
                    .get::<_, Option<String>>(20)?
                    .and_then(|m| serde_json::from_str(&m).ok()),
                stack_trace: row.get(21)?,
                synced_at: row
                    .get::<_, Option<String>>(22)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(23)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }

        Ok(events)
    }

    /// Get statistics
    pub fn stats(&self) -> Result<GovernanceStats> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;

        let total: u64 =
            conn.query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))?;

        let today = Utc::now().date_naive();
        let today_start = today.and_hms_opt(0, 0, 0).unwrap();
        let today_start = DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);

        let today_count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM audit_events WHERE timestamp >= ?",
            [today_start.to_rfc3339()],
            |row| row.get(0),
        )?;

        let errors: u64 = conn.query_row(
            "SELECT COUNT(*) FROM audit_events WHERE result = 'failure'",
            [],
            |row| row.get(0),
        )?;

        let mut stats = GovernanceStats {
            total,
            today: today_count,
            errors,
            ..Default::default()
        };

        // Get by level
        let mut stmt = conn.prepare("SELECT level, COUNT(*) FROM audit_events GROUP BY level")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;

        for row in rows {
            if let Ok((level, count)) = row {
                stats.by_level.insert(level, count);
            }
        }

        // Get top actions
        let mut stmt = conn.prepare(
            "SELECT action, COUNT(*) as cnt FROM audit_events GROUP BY action ORDER BY cnt DESC LIMIT 10",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TopAction {
                action: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        for row in rows {
            if let Ok(action) = row {
                stats.top_actions.push(action);
            }
        }

        Ok(stats)
    }

    /// Mark events as synced
    pub fn mark_synced(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE audit_events SET synced_at = ? WHERE id IN ({})",
            placeholders
        );

        let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&now];
        params_vec.extend(ids.iter().map(|s| s as &dyn rusqlite::ToSql));

        conn.execute(&sql, params_vec.as_slice())?;
        Ok(())
    }

    /// Cleanup old records
    pub fn cleanup(&self) -> Result<u64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;
        let cutoff = (Utc::now() - Duration::days(self.retention_days as i64)).to_rfc3339();

        let deleted = conn.execute("DELETE FROM audit_events WHERE timestamp < ?", [cutoff])?;

        Ok(deleted as u64)
    }

    /// Get count of unsynced events
    pub fn unsynced_count(&self) -> Result<u64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| GovernanceError::Database(e.to_string()))?;
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM audit_events WHERE synced_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_log() {
        let logger = AuditLogger::in_memory().unwrap();

        // Log an event
        let event = AuditEvent::success("test_action")
            .with_user("test_user")
            .with_message("Test message");

        logger.log(&event).unwrap();

        // Query events
        let filter = AuditFilter::new().action("test_action");
        let events = logger.query(&filter).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "test_action");
    }

    #[tokio::test]
    async fn test_audit_stats() {
        let logger = AuditLogger::in_memory().unwrap();

        logger.log(&AuditEvent::success("action1")).unwrap();
        logger.log(&AuditEvent::success("action2")).unwrap();
        logger.log(&AuditEvent::error("action3", "error")).unwrap();

        let stats = logger.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.errors, 1);
    }
}
