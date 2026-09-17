-- UP
-- Worklog entries — ingest target for `agileplus worklog emit`.
--
-- Captures the canonical task-worklog schema defined in
-- WORKLOG_SCHEMA_2026_06_10.md (status, task_id, agent_id, files_changed,
-- commit_sha, verification_result, started_at, completed_at).
--
-- `payload_json` preserves the full original worklog document so future
-- schema changes can re-parse historical entries without re-ingest.
--
-- `source_path` records where the worklog was loaded from for traceability.
CREATE TABLE IF NOT EXISTS worklog_entries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    status              TEXT    NOT NULL CHECK(status IN (
                            'pending','running','blocked',
                            'completed','failed','cancelled')),
    task_id             TEXT    NOT NULL,
    agent_id            TEXT    NOT NULL,
    files_changed_json  TEXT    NOT NULL DEFAULT '[]',
    commit_sha          TEXT,
    verification_json   TEXT    NOT NULL,
    started_at          TEXT    NOT NULL,
    completed_at        TEXT,
    source_path         TEXT    NOT NULL,
    payload_json        TEXT    NOT NULL,
    ingested_at         TEXT    NOT NULL,
    UNIQUE(task_id, source_path)
);

CREATE INDEX IF NOT EXISTS idx_worklog_entries_task_id
    ON worklog_entries (task_id);
CREATE INDEX IF NOT EXISTS idx_worklog_entries_agent_id
    ON worklog_entries (agent_id);
CREATE INDEX IF NOT EXISTS idx_worklog_entries_status
    ON worklog_entries (status);

-- DOWN
DROP INDEX IF EXISTS idx_worklog_entries_status;
DROP INDEX IF EXISTS idx_worklog_entries_agent_id;
DROP INDEX IF EXISTS idx_worklog_entries_task_id;
DROP TABLE IF EXISTS worklog_entries;
