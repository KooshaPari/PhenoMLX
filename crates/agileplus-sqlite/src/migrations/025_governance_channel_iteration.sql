-- UP
CREATE TABLE IF NOT EXISTS channel_iterations (
    channel_id TEXT NOT NULL,
    iteration INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (channel_id)
);

-- DOWN
DROP TABLE IF EXISTS channel_iterations;
