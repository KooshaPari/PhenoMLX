//! Epic repository functions.
//!
//! Traceability: FR-STORE-EPIC

use chrono::DateTime;
use rusqlite::{Connection, params};

use agileplus_domain::domain::epic::{Epic, EpicStatus};
use agileplus_domain::error::DomainError;

fn map_err(e: rusqlite::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

fn parse_dt(s: &str) -> DateTime<chrono::Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn row_to_epic(row: &rusqlite::Row<'_>) -> rusqlite::Result<Epic> {
    let status_str: String = row.get(4)?;
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;
    Ok(Epic {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: status_str.parse().unwrap_or(EpicStatus::Backlog),
        owner_id: row.get(5)?,
        requirement_id: None,
        created_at: parse_dt(&created_at),
        updated_at: parse_dt(&updated_at),
    })
}

/// Create an epic and return its new row ID.
pub fn create_epic(conn: &Connection, epic: &Epic) -> Result<i64, DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO epics (project_id, title, description, status, owner_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            epic.project_id,
            epic.title,
            epic.description,
            epic.status.to_string(),
            epic.owner_id,
            now,
            now,
        ],
    )
    .map_err(map_err)?;
    Ok(conn.last_insert_rowid())
}

pub fn get_epic_by_requirement_id(
    conn: &Connection,
    requirement_id: &str,
) -> Result<Option<Epic>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, title, description, status, owner_id, created_at, updated_at \
             FROM epics WHERE requirement_id = ?1",
        )
        .map_err(map_err)?;

    match stmt.query_row(params![requirement_id], row_to_epic) {
        Ok(mut epic) => {
            epic.requirement_id = Some(requirement_id.to_string());
            Ok(Some(epic))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

pub fn upsert_epic_by_requirement_id(conn: &Connection, epic: &Epic) -> Result<i64, DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    let req_id_owned = epic.requirement_id.clone();
    let Some(requirement_id) = req_id_owned.as_deref() else {
        return create_epic(conn, epic);
    };

    if let Some(existing) = get_epic_by_requirement_id(conn, requirement_id)? {
        return Ok(existing.id);
    }

    // Insert path: create_epic ignores requirement_id (a pre-existing bug). Insert
    // here directly so the row is keyed on requirement_id and get-by-requirement-id
    // can find it on the next call.
    conn.execute(
        "INSERT INTO epics (project_id, title, description, status, owner_id, requirement_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            epic.project_id,
            epic.title,
            epic.description,
            epic.status.to_string(),
            epic.owner_id,
            requirement_id,
            &now,
            &now,
        ],
    )
    .map_err(map_err)?;
    Ok(conn.last_insert_rowid())
}

/// Look up an epic by ID. Returns `None` if not found.
pub fn get_epic_by_id(conn: &Connection, id: i64) -> Result<Option<Epic>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, title, description, status, owner_id, created_at, updated_at \
             FROM epics WHERE id = ?1",
        )
        .map_err(map_err)?;

    match stmt.query_row(params![id], row_to_epic) {
        Ok(e) => Ok(Some(e)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

/// Update the status of an epic.
pub fn update_epic_status(conn: &Connection, id: i64, status: EpicStatus) -> Result<(), DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = conn
        .execute(
            "UPDATE epics SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("epic {id}")));
    }
    Ok(())
}

/// List all epics for a given project, ordered by creation time.
pub fn list_epics_by_project(conn: &Connection, project_id: i64) -> Result<Vec<Epic>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, title, description, status, owner_id, created_at, updated_at \
             FROM epics WHERE project_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(map_err)?;

    let epics = stmt
        .query_map(params![project_id], row_to_epic)
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    Ok(epics)
}

/// Delete an epic by ID. Will fail if stories reference it (FK constraint).
pub fn delete_epic(conn: &Connection, id: i64) -> Result<(), DomainError> {
    let rows = conn
        .execute("DELETE FROM epics WHERE id = ?1", params![id])
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("epic {id}")));
    }
    Ok(())
}
