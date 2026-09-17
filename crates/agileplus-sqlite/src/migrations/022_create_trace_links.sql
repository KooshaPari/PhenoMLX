-- UP
-- Trace links: directed edges between domain entities (work_package ↔
-- feature, work_package ↔ work_package, story ↔ epic, etc.).  Used by
-- `ap trace link <from> <to>` and rendered by `ap dashboard` as part
-- of the in-flight DAG view.
CREATE TABLE IF NOT EXISTS trace_links (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    from_kind  TEXT    NOT NULL CHECK(from_kind IN (
                  'work_package','feature','story','epic','project',
                  'cycle','module','requirement','external')),
    from_id    TEXT    NOT NULL,                 -- stringified id (e.g. "42")
    to_kind    TEXT    NOT NULL CHECK(to_kind IN (
                  'work_package','feature','story','epic','project',
                  'cycle','module','requirement','external')),
    to_id      TEXT    NOT NULL,                 -- stringified id
    link_type  TEXT    NOT NULL CHECK(link_type IN (
                  'parent_of','child_of','depends_on','blocks',
                  'implements','verifies','references','duplicates')),
    note       TEXT    NOT NULL DEFAULT '',
    created_by TEXT    NOT NULL DEFAULT 'system',
    created_at TEXT    NOT NULL,
    UNIQUE(from_kind, from_id, to_kind, to_id, link_type)
);

CREATE INDEX IF NOT EXISTS idx_trace_links_from
    ON trace_links(from_kind, from_id);
CREATE INDEX IF NOT EXISTS idx_trace_links_to
    ON trace_links(to_kind, to_id);
CREATE INDEX IF NOT EXISTS idx_trace_links_type
    ON trace_links(link_type);

-- DOWN
DROP INDEX IF EXISTS idx_trace_links_type;
DROP INDEX IF EXISTS idx_trace_links_to;
DROP INDEX IF EXISTS idx_trace_links_from;
DROP TABLE IF EXISTS trace_links;
