-- UP
CREATE TABLE IF NOT EXISTS stories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    epic_id     INTEGER NOT NULL REFERENCES epics(id),
    project_id  INTEGER NOT NULL REFERENCES projects(id),
    title       TEXT    NOT NULL,
    description TEXT,
    status      TEXT    NOT NULL DEFAULT 'todo',
    points      INTEGER,
    assignee_id INTEGER REFERENCES users(id),
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_stories_epic_id    ON stories (epic_id);
CREATE INDEX IF NOT EXISTS idx_stories_project_id ON stories (project_id);
CREATE INDEX IF NOT EXISTS idx_stories_status     ON stories (status);

-- DOWN
DROP INDEX IF EXISTS idx_stories_status;
DROP INDEX IF EXISTS idx_stories_project_id;
DROP INDEX IF EXISTS idx_stories_epic_id;
DROP TABLE IF EXISTS stories;
