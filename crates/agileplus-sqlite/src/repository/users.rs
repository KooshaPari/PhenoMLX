//! User repository functions.
//!
//! Traceability: FR-STORE-USER

use chrono::DateTime;
use rusqlite::{Connection, params};

use agileplus_domain::domain::user::{User, UserRole, UserStatus};
use agileplus_domain::error::DomainError;

fn map_err(e: rusqlite::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

fn parse_dt(s: &str) -> DateTime<chrono::Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn row_to_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    let role_str: String = row.get(3)?;
    let status_str: String = row.get(4)?;
    let created_at: String = row.get(7)?;
    let updated_at: String = row.get(8)?;
    Ok(User {
        id: row.get(0)?,
        display_name: row.get(1)?,
        email: row.get(2)?,
        role: role_str.parse().unwrap_or(UserRole::Member),
        status: status_str.parse().unwrap_or(UserStatus::Active),
        avatar_url: row.get(5)?,
        github_login: row.get(6)?,
        created_at: parse_dt(&created_at),
        updated_at: parse_dt(&updated_at),
    })
}

/// Create a user and return its new row ID.
pub fn create_user(conn: &Connection, user: &User) -> Result<i64, DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (display_name, email, role, status, avatar_url, github_login, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            user.display_name,
            user.email,
            user.role.to_string(),
            user.status.to_string(),
            user.avatar_url,
            user.github_login,
            now,
            now,
        ],
    )
    .map_err(map_err)?;
    Ok(conn.last_insert_rowid())
}

/// Look up a user by ID. Returns `None` if not found.
pub fn get_user_by_id(conn: &Connection, id: i64) -> Result<Option<User>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, display_name, email, role, status, avatar_url, github_login, created_at, updated_at \
             FROM users WHERE id = ?1",
        )
        .map_err(map_err)?;

    match stmt.query_row(params![id], row_to_user) {
        Ok(u) => Ok(Some(u)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

/// Look up a user by email. Returns `None` if not found.
pub fn get_user_by_email(conn: &Connection, email: &str) -> Result<Option<User>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, display_name, email, role, status, avatar_url, github_login, created_at, updated_at \
             FROM users WHERE email = ?1",
        )
        .map_err(map_err)?;

    match stmt.query_row(params![email], row_to_user) {
        Ok(u) => Ok(Some(u)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

/// Update the status of a user.
pub fn update_user_status(conn: &Connection, id: i64, status: UserStatus) -> Result<(), DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = conn
        .execute(
            "UPDATE users SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("user {id}")));
    }
    Ok(())
}

/// Update the role of a user.
pub fn update_user_role(conn: &Connection, id: i64, role: UserRole) -> Result<(), DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = conn
        .execute(
            "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
            params![role.to_string(), now, id],
        )
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("user {id}")));
    }
    Ok(())
}

/// List all users ordered by created_at.
pub fn list_all_users(conn: &Connection) -> Result<Vec<User>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, display_name, email, role, status, avatar_url, github_login, created_at, updated_at \
             FROM users ORDER BY created_at ASC",
        )
        .map_err(map_err)?;

    let users = stmt
        .query_map([], row_to_user)
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    Ok(users)
}

/// Delete a user by ID.
pub fn delete_user(conn: &Connection, id: i64) -> Result<(), DomainError> {
    let rows = conn
        .execute("DELETE FROM users WHERE id = ?1", params![id])
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("user {id}")));
    }
    Ok(())
}
