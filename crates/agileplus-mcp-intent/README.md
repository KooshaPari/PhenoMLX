# agileplus-mcp-intent

MCP tool + HTTP service for converting user prompts into structured AgilePlus intent graphs.

## Overview

This crate provides a lightweight, rule-based converter (no external LLM APIs) that transforms natural language prompts into intent graphs conforming to the AgilePlus Intent Graph Ontology.

## Interfaces

### 1. MCP Tool (stdio JSON-RPC)

Run the MCP server:

```bash
agileplus-mcp-intent mcp
```

Exposed tool:

- `convert_prompt_to_intent_graph(prompt: string, options?: object) -> object`

Example JSON-RPC request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "convert_prompt_to_intent_graph",
    "arguments": {
      "prompt": "Add dark mode to settings",
      "options": {
        "auto_decompose": true,
        "max_features": 5
      }
    }
  }
}
```

### 2. HTTP API

Run the HTTP server:

```bash
agileplus-mcp-intent http --port 8080
```

Endpoints:

| Method | Path | Description |
|--------|------|-------------|
| POST | `/convert` | Convert prompt to intent graph |
| POST | `/convert-and-store` | Convert and optionally store in DB |
| GET | `/health` | Health check |
| GET | `/schema` | Embedded ontology schema |

#### Example request

```bash
curl -X POST http://localhost:8080/convert \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Add dark mode to settings", "options": {"auto_decompose": true, "max_features": 5}}'
```

#### Example response

```json
{
  "graph": {
    "nodes": [
      {
        "id": "Intent#add-dark-mode-to-settings",
        "node_type": "Intent",
        "dag_stage": "intent",
        "title": "Add dark mode to settings",
        "status": "draft",
        "meta": {
          "timestamp": "2026-06-13T12:00:00Z",
          "source": "user-prompt",
          "confidence": 0.92,
          "agent_id": "agileplus-mcp-intent"
        },
        "properties": {
          "priority": "medium",
          "stakeholders": ["end-user", "designer"],
          "acceptance_criteria": ["UI element is visible to the user"],
          "auto_decomposed": true,
          "max_features": 5
        }
      },
      {
        "id": "Plan#add-dark-mode-to-settings-plan",
        "node_type": "Plan",
        "dag_stage": "plan",
        "title": "Plan for Add dark mode to settings",
        "status": "draft",
        "meta": {
          "timestamp": "2026-06-13T12:00:00Z",
          "source": "agent-inference",
          "confidence": 0.78,
          "agent_id": "agileplus-mcp-intent"
        }
      },
      {
        "id": "Feature#add-dark-mode-to-settings-feat-1",
        "node_type": "Feature",
        "dag_stage": "feature",
        "title": "Theming",
        "description": "UI appearance, themes, and color schemes",
        "status": "draft",
        "tags": ["ui", "theming"],
        "meta": {
          "timestamp": "2026-06-13T12:00:00Z",
          "source": "agent-inference",
          "confidence": 0.68,
          "agent_id": "agileplus-mcp-intent"
        }
      }
    ],
    "edges": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "source": "Intent#add-dark-mode-to-settings",
        "target": "Plan#add-dark-mode-to-settings-plan",
        "relationship_type": "implements",
        "canonical_map": {
          "link_type": "parent_of",
          "direction": "forward"
        },
        "meta": {
          "timestamp": "2026-06-13T12:00:00Z",
          "source": "agent-inference",
          "confidence": 0.78,
          "agent_id": "agileplus-mcp-intent"
        }
      }
    ],
    "metadata": {
      "version": "1.0.0",
      "schema_uri": "https://phenotype.dev/schemas/agileplus-intent-ontology/v1.json",
      "created_at": "2026-06-13T12:00:00Z",
      "updated_at": "2026-06-13T12:00:00Z",
      "node_count": 3,
      "edge_count": 1,
      "dag_valid": true,
      "source_system": "agileplus-mcp-intent"
    }
  },
  "summary": {
    "node_count": 3,
    "edge_count": 1,
    "intent_title": "Add dark mode to settings",
    "features_generated": 1,
    "plan_generated": true,
    "confidence": 0.79
  }
}
```

### 3. CLI (one-shot)

```bash
# Convert only
agileplus-mcp-intent convert "Add dark mode to settings"

# Convert and store in database
agileplus-mcp-intent convert --store "Add dark mode to settings"
```

### 4. Justfile recipes

```bash
# Convert a prompt
just convert-intent "Add dark mode to settings"

# Convert and store
just convert-intent-store "Add dark mode to settings"

# Run HTTP server
just run-intent-http 8080

# Run MCP server
just run-intent-mcp
```

## Conversion Logic

The converter performs the following steps:

1. **Intent extraction** — derives title, description, priority, stakeholders, and acceptance criteria from the prompt using keyword heuristics.
2. **Feature decomposition** — scans the prompt for known domain keywords (auth, theming, notifications, search, etc.) and auto-generates up to `max_features` Feature nodes.
3. **Plan generation** — creates a Plan node linked to the Intent.
4. **Edge creation** — links Intent → Plan (implements), Plan → Feature (implements), Feature → Intent (derives-from), and Intent → Feature/Story (traces-to).
5. **Confidence scoring** — each node/edge gets a confidence score in the range `[0.0, 1.0]` based on keyword match density.

## Validation

Every generated graph is validated against the embedded JSON Schema (`ontology.json`). The validator checks:

- Required fields (`nodes`, `edges`, `metadata`)
- Node ID format: `[A-Z][a-z]+#[a-z0-9\-]+`
- Edge meta blocks contain `timestamp`, `source`, `agent_id`, and `confidence`
- Ontology enum values for `node_type`, `dag_stage`, `relationship_type`, and `status`

## Error Codes

| Code | Meaning |
|------|---------|
| `CONVERSION_ERROR` | Rule-based conversion failed |
| `ONTOLOGY_VALIDATION_ERROR` | Output does not conform to the schema |
| `BAD_REQUEST` | Malformed input JSON or missing required fields |
| `PARSE_ERROR` | Invalid JSON-RPC request |
| `METHOD_NOT_FOUND` | Unknown JSON-RPC method |

## Database Integration

The `--store` flag (or `store: true` in options) writes Feature nodes to the AgilePlus SQLite database (`agileplus.db` by default, or `AGILEPLUS_DB` env var).

- `Intent` and `Plan` nodes are not stored (no direct tables yet).
- `Feature` nodes are mapped to the `features` table.
- `Story` nodes require an epic + project and are skipped unless a default project exists.

## Dependencies

- No external LLM APIs (rule-based only)
- Uses `jsonschema` for validation
- Uses `axum` for HTTP server
- Uses `agileplus-sqlite` for optional storage

## Schema Reference

The embedded ontology matches `~/forge/prompt-corpus/agileplus-intent-ontology.json`:

- Node types: `Intent`, `Plan`, `Feature`, `Story`, `Task`, `Spec`, `Commit`, `Test`, `PR`, `Bug`, `Artifact`
- Relationship types: `implements`, `tests`, `covers`, `traces-to`, `derives-from`, `resolves`, `blocks`, `depends-on`
- Canonical link types: `parent_of`, `child_of`, `depends_on`, `blocks`, `implements`, `verifies`, `references`, `duplicates`

## Traceability

- Edge `meta` blocks follow the pattern from `agileplus-subcmds/src/tracera_bridge.rs` (confidence scoring, agent_id, timestamp).
- Node IDs follow the ontology slug format used across AgilePlus.
