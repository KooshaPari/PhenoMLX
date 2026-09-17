-- UP
-- Migration 025: Intent Graph Ontology tables
-- Maps the agileplus-intent-ontology JSON schema to SQLite.
--
-- Node types: Intent, Plan, Feature, Story, Task, Spec, Commit, Test, PR, Bug, Artifact
-- Edge types: implements, tests, covers, traces-to, derives-from, resolves, blocks, depends-on
-- Status values: draft, active, completed, deprecated, rejected, open, in_progress, blocked, deferred, cancelled
-- DAG stages: intent, plan, feature, story, task, spec, commit, test, pr, bug, artifact

-- Nodes: typed entities in the intent graph.
CREATE TABLE IF NOT EXISTS intent_nodes (
    id           TEXT PRIMARY KEY,
    node_type    TEXT NOT NULL CHECK(node_type IN (
                     'Intent','Plan','Feature','Story','Task',
                     'Spec','Commit','Test','PR','Bug','Artifact')),
    dag_stage    TEXT NOT NULL CHECK(dag_stage IN (
                     'intent','plan','feature','story','task',
                     'spec','commit','test','pr','bug','artifact')),
    title        TEXT NOT NULL,
    description  TEXT,
    status       TEXT NOT NULL CHECK(status IN (
                     'draft','active','completed','deprecated','rejected',
                     'open','in_progress','blocked','deferred','cancelled')),
    tags         TEXT,    -- JSON array
    meta         TEXT,    -- JSON object (required: timestamp, source)
    properties   TEXT,    -- JSON object
    table_ref    TEXT,
    table_id     TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- Edges: directed relationships between intent nodes.
CREATE TABLE IF NOT EXISTS intent_edges (
    id                TEXT PRIMARY KEY,
    source            TEXT NOT NULL REFERENCES intent_nodes(id) ON DELETE CASCADE,
    target            TEXT NOT NULL REFERENCES intent_nodes(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL CHECK(relationship_type IN (
                          'implements','tests','covers','traces-to',
                          'derives-from','resolves','blocks','depends-on')),
    canonical_map     TEXT,    -- JSON object (link_type, direction)
    meta              TEXT,    -- JSON object (required: timestamp, source)
    properties        TEXT,    -- JSON object
    created_at        TEXT NOT NULL
);

-- Metadata: singleton-like graph metadata row.
CREATE TABLE IF NOT EXISTS intent_graph_metadata (
    id             INTEGER PRIMARY KEY,
    version        TEXT,
    schema_uri     TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT,
    node_count     INTEGER,
    edge_count     INTEGER,
    dag_valid      INTEGER CHECK(dag_valid IN (0,1)),  -- BOOLEAN
    source_system  TEXT
);

-- Indexes for graph traversal and filtering.
CREATE INDEX IF NOT EXISTS idx_intent_edges_source
    ON intent_edges(source);
CREATE INDEX IF NOT EXISTS idx_intent_edges_target
    ON intent_edges(target);
CREATE INDEX IF NOT EXISTS idx_intent_edges_relationship_type
    ON intent_edges(relationship_type);

CREATE INDEX IF NOT EXISTS idx_intent_nodes_node_type
    ON intent_nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_intent_nodes_status
    ON intent_nodes(status);
CREATE INDEX IF NOT EXISTS idx_intent_nodes_dag_stage
    ON intent_nodes(dag_stage);

-- Validation triggers supplement the CHECK constraints above.
-- They fire on INSERT/UPDATE to enforce ontology enum membership.
CREATE TRIGGER IF NOT EXISTS trg_intent_nodes_validate_type
BEFORE INSERT ON intent_nodes
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.node_type NOT IN (
            'Intent','Plan','Feature','Story','Task',
            'Spec','Commit','Test','PR','Bug','Artifact'
        ) THEN RAISE(ABORT, 'Invalid node_type')
    END;
END;

CREATE TRIGGER IF NOT EXISTS trg_intent_nodes_validate_status
BEFORE INSERT ON intent_nodes
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.status NOT IN (
            'draft','active','completed','deprecated','rejected',
            'open','in_progress','blocked','deferred','cancelled'
        ) THEN RAISE(ABORT, 'Invalid status')
    END;
END;

CREATE TRIGGER IF NOT EXISTS trg_intent_edges_validate_type
BEFORE INSERT ON intent_edges
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.relationship_type NOT IN (
            'implements','tests','covers','traces-to',
            'derives-from','resolves','blocks','depends-on'
        ) THEN RAISE(ABORT, 'Invalid relationship_type')
    END;
END;

-- DOWN
DROP TRIGGER IF EXISTS trg_intent_edges_validate_type;
DROP TRIGGER IF EXISTS trg_intent_nodes_validate_status;
DROP TRIGGER IF EXISTS trg_intent_nodes_validate_type;

DROP INDEX IF EXISTS idx_intent_nodes_dag_stage;
DROP INDEX IF EXISTS idx_intent_nodes_status;
DROP INDEX IF EXISTS idx_intent_nodes_node_type;
DROP INDEX IF EXISTS idx_intent_edges_relationship_type;
DROP INDEX IF EXISTS idx_intent_edges_target;
DROP INDEX IF EXISTS idx_intent_edges_source;

DROP TABLE IF EXISTS intent_graph_metadata;
DROP TABLE IF EXISTS intent_edges;
DROP TABLE IF EXISTS intent_nodes;
