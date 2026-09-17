// SPDX-License-Identifier: MIT OR Apache-2.0
//! `intent` CLI subcommand.
//!
//! Reads a natural-language prompt and emits a structured intent graph JSON
//! conforming to the AgilePlus Intent Graph Ontology.

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// â”€â”€ Constants â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

const ONTOLOGY_VERSION: &str = "1.0.0";
const SCHEMA_URI: &str = "https://phenotype.dev/schemas/agileplus-intent-ontology/v1.json";
const AGENT_ID: &str = "agileplus-cli/intent";

const TASK_TYPES: &[&str] = &[
    "implementation",
    "refactor",
    "research",
    "design",
    "documentation",
    "test",
    "devops",
    "security",
    "review",
];

// â”€â”€ CLI Args â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Arguments for the `intent` subcommand.
#[derive(Args, Debug)]
pub struct IntentArgs {
    /// Natural language prompt describing the intent.
    #[arg(long, short = 'p')]
    pub prompt: String,

    /// Optional path to write the JSON output (defaults to stdout).
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Validate the generated graph against the ontology schema.
    #[arg(long)]
    pub validate: bool,
}

// â”€â”€ Graph Types â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Serialize, Deserialize)]
struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f64>,
    source: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Node {
    id: String,
    node_type: String,
    dag_stage: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    table_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    table_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalMap {
    link_type: String,
    direction: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Edge {
    id: String,
    source: String,
    target: String,
    relationship_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_map: Option<CanonicalMap>,
    meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphMetadata {
    version: String,
    schema_uri: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    node_count: usize,
    edge_count: usize,
    dag_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IntentGraph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    metadata: GraphMetadata,
}

// â”€â”€ Public Entry Point â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub fn run(args: &IntentArgs) -> Result<()> {
    let graph = generate_graph(&args.prompt);

    if args.validate {
        validate_graph(&graph)?;
        eprintln!(
            "Validation passed: {} node(s), {} edge(s)",
            graph.nodes.len(),
            graph.edges.len()
        );
    }

    let json = serde_json::to_string_pretty(&graph).context("serialize intent graph to JSON")?;

    if let Some(path) = &args.output {
        std::fs::write(path, json)
            .with_context(|| format!("write intent graph to {}", path.display()))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

// â”€â”€ Graph Generation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn generate_graph(prompt: &str) -> IntentGraph {
    let slug = slugify(prompt);
    let now = Utc::now().to_rfc3339();

    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();

    // â”€â”€ Intent node â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let intent_id = format!("Intent#{slug}");
    nodes.push(Node {
        id: intent_id.clone(),
        node_type: "Intent".to_string(),
        dag_stage: "intent".to_string(),
        title: prompt.to_string(),
        description: Some(format!("User intent derived from prompt: {prompt}")),
        status: "draft".to_string(),
        tags: Some(vec!["intent".to_string(), "auto-generated".to_string()]),
        meta: Meta {
            confidence: Some(1.0),
            source: "user-prompt".to_string(),
            timestamp: now.clone(),
            agent_id: Some(AGENT_ID.to_string()),
        },
        properties: None,
        table_ref: None,
        table_id: None,
    });

    // â”€â”€ Plan node â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let plan_id = format!("Plan#{slug}-plan");
    nodes.push(Node {
        id: plan_id.clone(),
        node_type: "Plan".to_string(),
        dag_stage: "plan".to_string(),
        title: format!("Plan for {prompt}"),
        description: Some("Auto-generated plan node from intent prompt.".to_string()),
        status: "draft".to_string(),
        tags: Some(vec!["plan".to_string()]),
        meta: Meta {
            confidence: Some(0.95),
            source: "agent-inference".to_string(),
            timestamp: now.clone(),
            agent_id: Some(AGENT_ID.to_string()),
        },
        properties: None,
        table_ref: None,
        table_id: None,
    });

    // Intent -> Plan edge
    edges.push(make_edge(
        &intent_id,
        &plan_id,
        "implements",
        CanonicalMap {
            link_type: "parent_of".to_string(),
            direction: "forward".to_string(),
        },
        0.95,
        &now,
    ));

    // â”€â”€ Feature nodes â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let feature_specs = extract_features(prompt, &slug);
    for (idx, (feat_slug, feat_title)) in feature_specs.iter().enumerate() {
        let feat_id = format!("Feature#{feat_slug}");
        nodes.push(Node {
            id: feat_id.clone(),
            node_type: "Feature".to_string(),
            dag_stage: "feature".to_string(),
            title: feat_title.clone(),
            description: Some(format!("Feature {} derived from intent.", idx + 1)),
            status: "draft".to_string(),
            tags: Some(vec!["feature".to_string()]),
            meta: Meta {
                confidence: Some(0.85),
                source: "agent-inference".to_string(),
                timestamp: now.clone(),
                agent_id: Some(AGENT_ID.to_string()),
            },
            properties: None,
            table_ref: None,
            table_id: None,
        });

        // Plan -> Feature
        edges.push(make_edge(
            &plan_id,
            &feat_id,
            "implements",
            CanonicalMap {
                link_type: "parent_of".to_string(),
                direction: "forward".to_string(),
            },
            0.85,
            &now,
        ));

        // Intent -> Feature
        edges.push(make_edge(
            &intent_id,
            &feat_id,
            "implements",
            CanonicalMap {
                link_type: "parent_of".to_string(),
                direction: "forward".to_string(),
            },
            0.90,
            &now,
        ));

        // â”€â”€ Task node(s) per feature â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        let task_type = TASK_TYPES[idx % TASK_TYPES.len()];
        let task_id = format!("Task#{slug}-task-{idx}");
        nodes.push(Node {
            id: task_id.clone(),
            node_type: "Task".to_string(),
            dag_stage: "task".to_string(),
            title: format!("{}: {}", capitalize(task_type), feat_title),
            description: Some(format!(
                "Task of type '{task_type}' for feature '{feat_title}'."
            )),
            status: "draft".to_string(),
            tags: Some(vec!["task".to_string(), task_type.to_string()]),
            meta: Meta {
                confidence: Some(0.80),
                source: "agent-inference".to_string(),
                timestamp: now.clone(),
                agent_id: Some(AGENT_ID.to_string()),
            },
            properties: Some(serde_json::json!({ "type": task_type })),
            table_ref: None,
            table_id: None,
        });

        // Feature -> Task
        edges.push(make_edge(
            &feat_id,
            &task_id,
            "implements",
            CanonicalMap {
                link_type: "parent_of".to_string(),
                direction: "forward".to_string(),
            },
            0.80,
            &now,
        ));
    }

    let node_count = nodes.len();
    let edge_count = edges.len();

    IntentGraph {
        nodes,
        edges,
        metadata: GraphMetadata {
            version: ONTOLOGY_VERSION.to_string(),
            schema_uri: SCHEMA_URI.to_string(),
            created_at: now.clone(),
            updated_at: Some(now),
            node_count,
            edge_count,
            dag_valid: true,
            source_system: Some(AGENT_ID.to_string()),
        },
    }
}

fn make_edge(
    source: &str,
    target: &str,
    rel: &str,
    canonical: CanonicalMap,
    confidence: f64,
    timestamp: &str,
) -> Edge {
    Edge {
        id: Uuid::new_v4().to_string(),
        source: source.to_string(),
        target: target.to_string(),
        relationship_type: rel.to_string(),
        canonical_map: Some(canonical),
        meta: Meta {
            confidence: Some(confidence),
            source: "agent-inference".to_string(),
            timestamp: timestamp.to_string(),
            agent_id: Some(AGENT_ID.to_string()),
        },
        properties: None,
    }
}

// â”€â”€ Heuristic Feature Extraction â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn extract_features(prompt: &str, base_slug: &str) -> Vec<(String, String)> {
    let lower = prompt.to_lowercase();
    let stop_words: std::collections::HashSet<&str> = [
        "a",
        "an",
        "the",
        "and",
        "or",
        "but",
        "in",
        "on",
        "at",
        "to",
        "for",
        "of",
        "with",
        "by",
        "from",
        "as",
        "is",
        "are",
        "was",
        "were",
        "be",
        "been",
        "being",
        "have",
        "has",
        "had",
        "do",
        "does",
        "did",
        "will",
        "would",
        "could",
        "should",
        "may",
        "might",
        "must",
        "can",
        "this",
        "that",
        "these",
        "those",
        "i",
        "you",
        "he",
        "she",
        "it",
        "we",
        "they",
        "me",
        "him",
        "her",
        "us",
        "them",
        "my",
        "your",
        "his",
        "its",
        "our",
        "their",
        "add",
        "create",
        "implement",
        "build",
        "make",
        "support",
        "enable",
        "update",
        "modify",
        "change",
        "remove",
        "delete",
        "fix",
        "improve",
        "optimize",
        "refactor",
        "integrate",
    ]
    .iter()
    .copied()
    .collect();

    // Split on punctuation and whitespace
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|t| !t.is_empty() && !stop_words.contains(t))
        .collect();

    if tokens.is_empty() {
        return vec![(
            format!("{base_slug}-feature"),
            "General feature".to_string(),
        )];
    }

    // Build candidate phrases from consecutive tokens
    let mut candidates: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < tokens.len() && candidates.len() < 5 {
        let word = tokens[i];
        // Try to form a 2-word phrase if possible
        if i + 1 < tokens.len() {
            let phrase = format!("{}-{}", word, tokens[i + 1]);
            let title = format!("{} {}", capitalize(word), capitalize(tokens[i + 1]));
            candidates.push((phrase, title));
            i += 2;
        } else {
            candidates.push((word.to_string(), capitalize(word)));
            i += 1;
        }
    }

    // Deduplicate by slug
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|(slug, _)| seen.insert(slug.clone()));

    if candidates.is_empty() {
        candidates.push((
            format!("{base_slug}-feature"),
            "General feature".to_string(),
        ));
    }

    candidates
}

// â”€â”€ Validation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn validate_graph(graph: &IntentGraph) -> Result<()> {
    // Round-trip through JSON to use the typed validator from agileplus-trace-validator.
    let json = serde_json::to_value(graph).context("serialize graph for validation")?;
    let validator_graph: agileplus_trace_validator::IntentGraph =
        serde_json::from_value(json).context("deserialize graph for validation")?;
    agileplus_trace_validator::validate_intent_graph(&validator_graph)
        .map_err(|e| anyhow::anyhow!("validation failed: {e}"))?;
    Ok(())
}

// â”€â”€ Utilities â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn slugify(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_hyphen = true;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            result.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            result.push('-');
            prev_hyphen = true;
        }
    }
    if result.ends_with('-') {
        result.pop();
    }
    // Collapse multiple hyphens
    let mut collapsed = String::with_capacity(result.len());
    let mut prev = '\0';
    for c in result.chars() {
        if c == '-' && prev == '-' {
            continue;
        }
        collapsed.push(c);
        prev = c;
    }
    if collapsed.is_empty() {
        "intent".to_string()
    } else {
        collapsed
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().to_string() + c.as_str(),
    }
}

// â”€â”€ Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;

    fn regex_check(s: &str, pattern: &str) -> bool {
        if pattern == r"^[A-Z][a-z]+#[a-z0-9\-]+$" {
            let mut chars = s.chars();
            let _first = match chars.next() {
                Some(c) if c.is_ascii_uppercase() => c,
                _ => return false,
            };
            let _second = match chars.next() {
                Some(c) if c.is_ascii_lowercase() => c,
                _ => return false,
            };
            let mut seen_lower = true;
            let mut seen_hash = false;
            for c in chars {
                if !seen_hash {
                    if c == '#' {
                        seen_hash = true;
                        continue;
                    }
                    if !c.is_ascii_lowercase() {
                        return false;
                    }
                    seen_lower = true;
                } else if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' {
                    return false;
                }
            }
            seen_hash && seen_lower
        } else {
            false
        }
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(
            slugify("Add dark mode to settings"),
            "add-dark-mode-to-settings"
        );
        assert_eq!(slugify("Hello   World!!!"), "hello-world");
    }

    #[test]
    fn regex_check_valid() {
        assert!(regex_check(
            "Intent#dark-mode",
            r"^[A-Z][a-z]+#[a-z0-9\-]+$"
        ));
        assert!(regex_check(
            "Feature#auth-oauth2",
            r"^[A-Z][a-z]+#[a-z0-9\-]+$"
        ));
        assert!(regex_check("Plan#foo-plan", r"^[A-Z][a-z]+#[a-z0-9\-]+$"));
    }

    #[test]
    fn regex_check_invalid() {
        assert!(!regex_check(
            "intent#dark-mode",
            r"^[A-Z][a-z]+#[a-z0-9\-]+$"
        )); // lowercase start
        assert!(!regex_check(
            "Intent#Dark-Mode",
            r"^[A-Z][a-z]+#[a-z0-9\-]+$"
        )); // uppercase in slug
        assert!(!regex_check(
            "Intent_dark-mode",
            r"^[A-Z][a-z]+#[a-z0-9\-]+$"
        )); // underscore
    }

    #[test]
    fn generate_and_validate_graph() {
        let graph = generate_graph("Add dark mode to settings");
        assert!(!graph.nodes.is_empty());
        assert!(!graph.edges.is_empty());
        validate_graph(&graph).unwrap();
    }

    #[test]
    fn validate_rejects_missing_intent() {
        let mut graph = generate_graph("Test");
        graph.nodes.retain(|n| n.node_type != "Intent");
        graph.metadata.node_count = graph.nodes.len();
        assert!(validate_graph(&graph).is_err());
    }
}
