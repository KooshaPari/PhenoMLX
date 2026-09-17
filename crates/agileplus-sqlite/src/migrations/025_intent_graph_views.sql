-- Helper SQL: Intent Graph Views
-- Run manually after migration 025 or include in application setup.
-- These views depend on tables created by 025_create_intent_graph.sql.

-- v_intent_graph: flat join of edges with source/target node details and graph counts.
CREATE VIEW IF NOT EXISTS v_intent_graph AS
SELECT
    e.id                AS edge_id,
    e.relationship_type,
    e.canonical_map,
    e.meta              AS edge_meta,
    e.properties        AS edge_properties,
    e.created_at        AS edge_created_at,
    src.id              AS source_id,
    src.node_type       AS source_type,
    src.title           AS source_title,
    src.status          AS source_status,
    tgt.id              AS target_id,
    tgt.node_type       AS target_type,
    tgt.title           AS target_title,
    tgt.status          AS target_status,
    (SELECT COUNT(*) FROM intent_nodes) AS total_nodes,
    (SELECT COUNT(*) FROM intent_edges) AS total_edges,
    (SELECT dag_valid FROM intent_graph_metadata WHERE id = 1) AS dag_valid
FROM intent_edges e
JOIN intent_nodes src ON src.id = e.source
JOIN intent_nodes tgt ON tgt.id = e.target;

-- v_traceability_chain: recursive trace from Intent nodes through all downstream edges.
-- Normalizes edge direction so that 'derives-from' is traversed parent -> child.
CREATE VIEW IF NOT EXISTS v_traceability_chain AS
WITH RECURSIVE trace_edge(from_node, to_node, relationship_type, edge_id) AS (
    -- Forward edges: source -> target
    SELECT source, target, relationship_type, id
    FROM intent_edges
    WHERE relationship_type IN (
        'implements','traces-to','blocks','depends-on','tests','covers','resolves'
    )
    UNION ALL
    -- Reverse derives-from: target (parent) -> source (child)
    SELECT target, source, relationship_type, id
    FROM intent_edges
    WHERE relationship_type = 'derives-from'
),
chain AS (
    -- Seed: start from Intent nodes
    SELECT
        n.id AS node_id,
        n.node_type,
        n.dag_stage,
        n.title,
        n.status,
        0 AS depth,
        n.id AS root_id,
        CAST(n.id AS TEXT) AS path
    FROM intent_nodes n
    WHERE n.node_type = 'Intent'

    UNION ALL

    -- Recurse: follow normalized trace edges
    SELECT
        n.id AS node_id,
        n.node_type,
        n.dag_stage,
        n.title,
        n.status,
        c.depth + 1,
        c.root_id,
        c.path || ' -> ' || n.id
    FROM chain c
    JOIN trace_edge te ON te.from_node = c.node_id
    JOIN intent_nodes n ON n.id = te.to_node
    WHERE c.depth < 10
)
SELECT * FROM chain;

-- v_orphan_nodes: nodes with no incoming or outgoing edges.
CREATE VIEW IF NOT EXISTS v_orphan_nodes AS
SELECT n.*
FROM intent_nodes n
LEFT JOIN intent_edges e_src ON e_src.source = n.id
LEFT JOIN intent_edges e_tgt ON e_tgt.target = n.id
WHERE e_src.id IS NULL AND e_tgt.id IS NULL;
