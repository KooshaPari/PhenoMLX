-- UP
-- Additive migration: external Tracera requirement ID cross-reference.
-- NULL means no requirement is linked (all existing rows get NULL).
ALTER TABLE epics   ADD COLUMN requirement_id TEXT;
ALTER TABLE stories ADD COLUMN requirement_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_epics_requirement_id
    ON epics (requirement_id) WHERE requirement_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_stories_requirement_id
    ON stories (requirement_id) WHERE requirement_id IS NOT NULL;

-- DOWN
DROP INDEX IF EXISTS idx_stories_requirement_id;
DROP INDEX IF EXISTS idx_epics_requirement_id;
-- SQLite does not support DROP COLUMN in older versions; columns are left nullable.
