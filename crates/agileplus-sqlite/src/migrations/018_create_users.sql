-- UP
CREATE TABLE IF NOT EXISTS users (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT    NOT NULL,
    email        TEXT    NOT NULL UNIQUE,
    role         TEXT    NOT NULL DEFAULT 'member',
    status       TEXT    NOT NULL DEFAULT 'active',
    avatar_url   TEXT,
    github_login TEXT,
    created_at   TEXT    NOT NULL,
    updated_at   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users (email);

-- DOWN
DROP INDEX IF EXISTS idx_users_email;
DROP TABLE IF EXISTS users;
