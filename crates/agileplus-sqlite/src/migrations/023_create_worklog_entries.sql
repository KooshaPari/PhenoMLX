-- UP
-- Worklog entries: a normalized, queryable projection of the
-- `worklogs/*.json` corpus (per `WORKLOG_SCHEMA_2026_06_10.md`).
-- `ap worklog emit` (L2 #39) writes rows here; `ap dashboard`
-- (L2 #40) renders the most-recent N rows as part of the in-flight
-- DAG view.
CREATE TABLE IF NOT EXISTS worklog_entries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id             TEXT    NOT NULL,             -- e.g. "L2-40"
    agent_id            TEXT    NOT NULL,
    status              TEXT    NOT NULL CHECK(status IN (
                            'pending','running','blocked',
                            'completed','failed','cancelled')),
    commit_sha          TEXT,                          -- nullable
    files_changed_json  TEXT    NOT NULL DEFAULT '[]', -- JSON array of paths
    verification_status TEXT    NOT NULL CHECK(verification_status IN (
                            'passed','failed','not_run','partial')) DEFAULT 'not_run',
    verification_notes  TEXT    NOT NULL DEFAULT '',
    verification_cmds   TEXT    NOT NULL DEFAULT '[]', -- JSON array
    verification_json   TEXT    NOT NULL DEFAULT '{"status":"not_run","commands":[],"notes":""}',
    started_at          TEXT,                          -- nullable (ISO-8601)
    completed_at        TEXT,                          -- nullable (ISO-8601)
    source_path         TEXT    NOT NULL DEFAULT '',
    payload_json        TEXT    NOT NULL DEFAULT '{}',
    ingested_at         TEXT    NOT NULL,
    UNIQUE(task_id, source_path)
);

CREATE INDEX IF NOT EXISTS idx_worklog_entries_task
    ON worklog_entries(task_id);
CREATE INDEX IF NOT EXISTS idx_worklog_entries_status
    ON worklog_entries(status);
CREATE INDEX IF NOT EXISTS idx_worklog_entries_completed
    ON worklog_entries(completed_at);

-- DOWN
DROP INDEX IF EXISTS idx_worklog_entries_completed;
DROP INDEX IF EXISTS idx_worklog_entries_status;
DROP INDEX IF EXISTS idx_worklog_entries_task;
DROP TABLE IF EXISTS worklog_entries;
