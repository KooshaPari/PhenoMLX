//! Story repository functions.
//!
//! Traceability: FR-STORE-STORY

use chrono::DateTime;
use rusqlite::{Connection, params};

use agileplus_domain::domain::story::{Story, StoryStatus};
use agileplus_domain::error::DomainError;

fn map_err(e: rusqlite::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

fn parse_dt(s: &str) -> DateTime<chrono::Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn row_to_story(row: &rusqlite::Row<'_>) -> rusqlite::Result<Story> {
    let status_str: String = row.get(4)?;
    let points_raw: Option<i64> = row.get(5)?;
    let created_at: String = row.get(7)?;
    let updated_at: String = row.get(8)?;
    Ok(Story {
        id: row.get(0)?,
        epic_id: row.get(1)?,
        project_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(9)?,
        status: status_str.parse().unwrap_or(StoryStatus::Todo),
        points: points_raw.map(|p| p as u32),
        assignee_id: row.get(6)?,
        requirement_id: None,
        created_at: parse_dt(&created_at),
        updated_at: parse_dt(&updated_at),
    })
}

/// Create a story and return its new row ID.
pub fn create_story(conn: &Connection, story: &Story) -> Result<i64, DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO stories (epic_id, project_id, title, description, status, points, assignee_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            story.epic_id,
            story.project_id,
            story.title,
            story.description,
            story.status.to_string(),
            story.points.map(|p| p as i64),
            story.assignee_id,
            now,
            now,
        ],
    )
    .map_err(map_err)?;
    Ok(conn.last_insert_rowid())
}

pub fn get_story_by_requirement_id(
    conn: &Connection,
    requirement_id: &str,
) -> Result<Option<Story>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epic_id, project_id, title, status, points, assignee_id, created_at, updated_at, description \
             FROM stories WHERE requirement_id = ?1",
        )
        .map_err(map_err)?;

    match stmt.query_row(params![requirement_id], row_to_story) {
        Ok(mut story) => {
            story.requirement_id = Some(requirement_id.to_string());
            Ok(Some(story))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

pub fn upsert_story_by_requirement_id(
    conn: &Connection,
    story: &Story,
) -> Result<i64, DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    let req_id_owned = story.requirement_id.clone();
    let Some(requirement_id) = req_id_owned.as_deref() else {
        return create_story(conn, story);
    };

    if let Some(existing) = get_story_by_requirement_id(conn, requirement_id)? {
        return Ok(existing.id);
    }

    // Insert path: create_story ignores requirement_id (same pre-existing bug as
    // upsert_epic_by_requirement_id). Insert here directly so the row is keyed on
    // requirement_id and get-by-requirement-id can find it on the next call.
    conn.execute(
        "INSERT INTO stories (epic_id, project_id, title, description, status, points, assignee_id, requirement_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            story.epic_id,
            story.project_id,
            story.title,
            story.description,
            story.status.to_string(),
            story.points.map(|p| p as i64),
            story.assignee_id,
            requirement_id,
            &now,
            &now,
        ],
    )
    .map_err(map_err)?;
    Ok(conn.last_insert_rowid())
}

/// Look up a story by ID. Returns `None` if not found.
pub fn get_story_by_id(conn: &Connection, id: i64) -> Result<Option<Story>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epic_id, project_id, title, status, points, assignee_id, created_at, updated_at, description \
             FROM stories WHERE id = ?1",
        )
        .map_err(map_err)?;

    match stmt.query_row(params![id], row_to_story) {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

/// Update the status of a story.
pub fn update_story_status(conn: &Connection, id: i64, status: StoryStatus) -> Result<(), DomainError> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = conn
        .execute(
            "UPDATE stories SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("story {id}")));
    }
    Ok(())
}

/// List all stories for a given epic, ordered by creation time.
pub fn list_stories_by_epic(conn: &Connection, epic_id: i64) -> Result<Vec<Story>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epic_id, project_id, title, status, points, assignee_id, created_at, updated_at, description \
             FROM stories WHERE epic_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(map_err)?;

    let stories = stmt
        .query_map(params![epic_id], row_to_story)
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    Ok(stories)
}

/// List all stories for a given project, ordered by creation time.
pub fn list_stories_by_project(conn: &Connection, project_id: i64) -> Result<Vec<Story>, DomainError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epic_id, project_id, title, status, points, assignee_id, created_at, updated_at, description \
             FROM stories WHERE project_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(map_err)?;

    let stories = stmt
        .query_map(params![project_id], row_to_story)
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    Ok(stories)
}

/// Delete a story by ID.
pub fn delete_story(conn: &Connection, id: i64) -> Result<(), DomainError> {
    let rows = conn
        .execute("DELETE FROM stories WHERE id = ?1", params![id])
        .map_err(map_err)?;
    if rows == 0 {
        return Err(DomainError::NotFound(format!("story {id}")));
    }
    Ok(())
}
