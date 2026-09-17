-- UP
CREATE TABLE IF NOT EXISTS epics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id),
    title       TEXT    NOT NULL,
    description TEXT,
    status      TEXT    NOT NULL DEFAULT 'backlog',
    owner_id    INTEGER REFERENCES users(id),
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_epics_project_id ON epics (project_id);
CREATE INDEX IF NOT EXISTS idx_epics_status     ON epics (status);

-- DOWN
DROP INDEX IF EXISTS idx_epics_status;
DROP INDEX IF EXISTS idx_epics_project_id;
DROP TABLE IF EXISTS epics;
