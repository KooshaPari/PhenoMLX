-- UP
-- L2 #38: Add traceability/observability tables identified as missing by the
-- L1 #5 audit. These tables back the gate, run, and scope surfaces used by
-- the agent / dashboard layers.
--
-- Notes:
--   * `worklog_entries` is already created by migration 023 (canonical
--     worklog ingest schema); it is NOT re-created here.
--   * `trace_links` is already created by migration 022 (DAG trace-link
--     schema); it is NOT re-created here.
--
-- Naming follows the existing conventions: plural snake_case, `id` is the
-- INTEGER PK, timestamps are RFC3339 TEXT and `created_at`/`updated_at`
-- are always populated. Foreign keys use `ON DELETE CASCADE` only where
-- the parent lifetime strictly outlives the child row.

-- Quality-gate evaluation results (clippy, tests, review, etc.).
-- One row per (work_package, gate, evaluation) — append-only on insert.
CREATE TABLE IF NOT EXISTS gate_results (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    work_package_id INTEGER NOT NULL REFERENCES work_packages(id) ON DELETE CASCADE,
    gate_name       TEXT    NOT NULL,
    status          TEXT    NOT NULL CHECK (status IN ('pass', 'fail', 'warn', 'skip', 'error')),
    evidence_ref    TEXT,
    payload         TEXT,
    checked_at      TEXT    NOT NULL,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gate_results_wp      ON gate_results (work_package_id);
CREATE INDEX IF NOT EXISTS idx_gate_results_gate    ON gate_results (gate_name);
CREATE INDEX IF NOT EXISTS idx_gate_results_status  ON gate_results (status);
CREATE INDEX IF NOT EXISTS idx_gate_results_checked ON gate_results (checked_at);

-- Run records for CI, test, build, lint, and other invocations.
-- `command` is the argv[0] / script name; `output` is a path to the log.
CREATE TABLE IF NOT EXISTS run_records (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    run_type     TEXT    NOT NULL,
    command      TEXT    NOT NULL,
    started_at   TEXT    NOT NULL,
    completed_at TEXT,
    status       TEXT    NOT NULL CHECK (status IN ('running', 'passed', 'failed', 'errored', 'cancelled')),
    exit_code    INTEGER,
    output       TEXT,
    metadata     TEXT,
    created_at   TEXT    NOT NULL,
    updated_at   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_run_records_type     ON run_records (run_type);
CREATE INDEX IF NOT EXISTS idx_run_records_status   ON run_records (status);
CREATE INDEX IF NOT EXISTS idx_run_records_started  ON run_records (started_at);

-- Per-file scope status for a work package. Tracks which files are
-- claimed, in-progress, completed, or blocked, scoped to a WP.
CREATE TABLE IF NOT EXISTS scope_status (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    work_package_id INTEGER NOT NULL REFERENCES work_packages(id) ON DELETE CASCADE,
    file_path       TEXT    NOT NULL,
    state           TEXT    NOT NULL CHECK (state IN ('claimed', 'in_progress', 'completed', 'blocked', 'released')),
    last_changed_by TEXT,
    last_changed_at TEXT,
    note            TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL,
    UNIQUE (work_package_id, file_path)
);

CREATE INDEX IF NOT EXISTS idx_scope_status_wp    ON scope_status (work_package_id);
CREATE INDEX IF NOT EXISTS idx_scope_status_state ON scope_status (state);

-- DOWN
-- Reverse order: drop tables that reference `work_packages` last
-- (they have ON DELETE CASCADE; we still drop in the right order so
-- that no FK is violated during teardown).
DROP INDEX IF EXISTS idx_scope_status_state;
DROP INDEX IF EXISTS idx_scope_status_wp;
DROP TABLE IF EXISTS scope_status;

DROP INDEX IF EXISTS idx_run_records_started;
DROP INDEX IF EXISTS idx_run_records_status;
DROP INDEX IF EXISTS idx_run_records_type;
DROP TABLE IF EXISTS run_records;

DROP INDEX IF EXISTS idx_gate_results_checked;
DROP INDEX IF EXISTS idx_gate_results_status;
DROP INDEX IF EXISTS idx_gate_results_gate;
DROP INDEX IF EXISTS idx_gate_results_wp;
DROP TABLE IF EXISTS gate_results;

-- Note: trace_links is owned by migration 022; do NOT drop here.
-- Note: worklog_entries is owned by migration 023; do NOT drop here.
