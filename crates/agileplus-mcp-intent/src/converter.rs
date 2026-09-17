// SPDX-License-Identifier: MIT OR Apache-2.0
//! Rule-based prompt -> intent graph conversion engine.
//!
//! No external LLM APIs are used.  Extraction is heuristic-driven
//! (keyword matching, sentence splitting, regex) with confidence scoring.

use chrono::Utc;
use regex::Regex;
use uuid::Uuid;

use crate::types::{
    make_edge, round_confidence, CanonicalLinkType, ConversionSummary, ConvertOptions,
    ConvertResponse, Direction, IntentGraph, Meta, Node, NodeType, RelationshipType, Status,
};

/// Convert a free-text prompt into a structured intent graph.
pub fn convert(prompt: &str, options: &ConvertOptions) -> anyhow::Result<ConvertResponse> {
    let slug = slugify(prompt);
    let intent_id = format!("Intent#{}", &slug);

    let title = extract_title(prompt);
    let description = if prompt.len() > title.len() + 10 {
        Some(prompt.to_string())
    } else {
        None
    };

    let priority = extract_priority(prompt);
    let stakeholders = extract_stakeholders(prompt);
    let acceptance = extract_acceptance_criteria(prompt);

    let intent_meta = Meta {
        timestamp: Utc::now(),
        source: "user-prompt".to_string(),
        confidence: Some(round_confidence(0.92)),
        agent_id: Some("agileplus-mcp-intent".to_string()),
    };

    let mut intent = Node::builder(&intent_id, NodeType::Intent, &title)
        .status(Status::Draft)
        .meta(intent_meta);

    if let Some(desc) = description {
        intent = intent.description(desc);
    }

    let mut properties = serde_json::Map::new();
    properties.insert("priority".to_string(), serde_json::json!(priority));
    if !stakeholders.is_empty() {
        properties.insert("stakeholders".to_string(), serde_json::json!(stakeholders));
    }
    if !acceptance.is_empty() {
        properties.insert(
            "acceptance_criteria".to_string(),
            serde_json::json!(acceptance),
        );
    }
    properties.insert(
        "auto_decomposed".to_string(),
        serde_json::json!(options.auto_decompose),
    );
    properties.insert(
        "max_features".to_string(),
        serde_json::json!(options.max_features),
    );
    intent = intent.properties(serde_json::Value::Object(properties));

    let mut nodes: Vec<Node> = vec![intent.build()];
    let mut edges: Vec<crate::types::Edge> = vec![];

    // -- Plan node --
    let plan_id = format!("Plan#{}-plan", &slug);
    let plan = Node::builder(&plan_id, NodeType::Plan, &format!("Plan for {title}"))
        .description(format!("Auto-generated plan from prompt: {prompt}"))
        .meta(Meta {
            timestamp: Utc::now(),
            source: "agent-inference".to_string(),
            confidence: Some(round_confidence(0.78)),
            agent_id: Some("agileplus-mcp-intent".to_string()),
        })
        .build();
    nodes.push(plan);

    edges.push(make_edge(
        &Uuid::new_v4().to_string(),
        &intent_id,
        &plan_id,
        RelationshipType::Implements,
        CanonicalLinkType::ParentOf,
        Direction::Forward,
        0.78,
    ));

    // -- Feature decomposition --
    let features = if options.auto_decompose {
        decompose_features(prompt, &slug, options.max_features)
    } else {
        vec![]
    };

    for (idx, feature) in features.iter().enumerate() {
        let fid = format!("Feature#{}-feat-{}", &slug, idx + 1);
        let fnode = Node::builder(&fid, NodeType::Feature, &feature.title)
            .description(feature.description.clone())
            .tags(feature.tags.clone())
            .meta(Meta {
                timestamp: Utc::now(),
                source: "agent-inference".to_string(),
                confidence: Some(round_confidence(feature.confidence)),
                agent_id: Some("agileplus-mcp-intent".to_string()),
            })
            .build();
        nodes.push(fnode);

        edges.push(make_edge(
            &Uuid::new_v4().to_string(),
            &plan_id,
            &fid,
            RelationshipType::Implements,
            CanonicalLinkType::ParentOf,
            Direction::Forward,
            feature.confidence,
        ));

        edges.push(make_edge(
            &Uuid::new_v4().to_string(),
            &fid,
            &intent_id,
            RelationshipType::DerivesFrom,
            CanonicalLinkType::ChildOf,
            Direction::Reverse,
            feature.confidence,
        ));
    }

    // -- Story node (if small scope) --
    if features.is_empty() {
        let story_id = format!("Story#{}-story", &slug);
        let story = Node::builder(&story_id, NodeType::Story, &format!("As a user, {title}"))
            .description(format!("User story derived from prompt: {prompt}"))
            .meta(Meta {
                timestamp: Utc::now(),
                source: "agent-inference".to_string(),
                confidence: Some(round_confidence(0.75)),
                agent_id: Some("agileplus-mcp-intent".to_string()),
            })
            .build();
        nodes.push(story);

        edges.push(make_edge(
            &Uuid::new_v4().to_string(),
            &plan_id,
            &story_id,
            RelationshipType::Implements,
            CanonicalLinkType::ParentOf,
            Direction::Forward,
            0.75,
        ));
        edges.push(make_edge(
            &Uuid::new_v4().to_string(),
            &story_id,
            &intent_id,
            RelationshipType::DerivesFrom,
            CanonicalLinkType::ChildOf,
            Direction::Reverse,
            0.75,
        ));
    }

    // -- Trace link edges: intent -> features (traces-to) --
    for node in &nodes {
        if matches!(node.node_type, NodeType::Feature | NodeType::Story) {
            edges.push(make_edge(
                &Uuid::new_v4().to_string(),
                &intent_id,
                &node.id,
                RelationshipType::TracesTo,
                CanonicalLinkType::References,
                Direction::Forward,
                0.70,
            ));
        }
    }

    let graph = IntentGraph::new(nodes, edges);
    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    let features_generated = graph
        .nodes
        .iter()
        .filter(|n| matches!(n.node_type, NodeType::Feature))
        .count();
    let plan_generated = graph
        .nodes
        .iter()
        .any(|n| matches!(n.node_type, NodeType::Plan));

    let avg_confidence = graph
        .nodes
        .iter()
        .filter_map(|n| n.meta.confidence)
        .sum::<f64>()
        / node_count.max(1) as f64;

    Ok(ConvertResponse {
        graph,
        summary: ConversionSummary {
            node_count,
            edge_count,
            intent_title: title,
            features_generated,
            plan_generated,
            confidence: avg_confidence,
        },
    })
}

/// Feature decomposition result.
struct FeatureDraft {
    title: String,
    description: String,
    tags: Vec<String>,
    confidence: f64,
}

/// Heuristic feature decomposition based on prompt keywords.
fn decompose_features(prompt: &str, _slug: &str, max: usize) -> Vec<FeatureDraft> {
    let lower = prompt.to_lowercase();
    let mut drafts: Vec<FeatureDraft> = vec![];

    // Keyword â†’ feature mapping
    let mut checks: Vec<(&[&str], &str, &str, Vec<&str>)> = vec![
        (
            &["auth", "login", "sign in", "signin", "sso", "oauth"],
            "Authentication",
            "User authentication and session management",
            vec!["auth", "security"],
        ),
        (
            &["dark mode", "theme", "light mode", "appearance"],
            "Theming",
            "UI appearance, themes, and color schemes",
            vec!["ui", "theming"],
        ),
        (
            &["settings", "preference", "config", "configuration"],
            "Settings",
            "User preferences and application configuration",
            vec!["ux", "settings"],
        ),
        (
            &["notification", "alert", "push", "email", "sms"],
            "Notifications",
            "Alert and notification delivery system",
            vec!["messaging", "notifications"],
        ),
        (
            &["search", "filter", "query", "find"],
            "Search",
            "Search and filtering capabilities",
            vec!["search", "ux"],
        ),
        (
            &["dashboard", "analytics", "metric", "report"],
            "Analytics",
            "Dashboards, metrics, and reporting",
            vec!["analytics", "data"],
        ),
        (
            &["api", "endpoint", "rest", "graphql", "grpc"],
            "API",
            "Backend API and integration endpoints",
            vec!["api", "backend"],
        ),
        (
            &["database", "sqlite", "postgres", "storage", "persist"],
            "Persistence",
            "Data storage and persistence layer",
            vec!["data", "backend"],
        ),
        (
            &["test", "testing", "spec", "unit test", "e2e"],
            "Testing",
            "Test coverage and quality assurance",
            vec!["qa", "testing"],
        ),
        (
            &["ci", "cd", "pipeline", "deploy", "github action"],
            "DevOps",
            "CI/CD pipelines and deployment automation",
            vec!["devops", "automation"],
        ),
    ];

    // Sort by relevance (number of matched keywords)
    checks.sort_by_key(|a| std::cmp::Reverse(a.0.len()));

    for (keywords, title, desc, tags) in &checks {
        if drafts.len() >= max {
            break;
        }
        if keywords.iter().any(|kw| lower.contains(kw)) {
            let already = drafts.iter().any(|d| d.title == *title);
            if !already {
                let matched = keywords.iter().filter(|kw| lower.contains(*kw)).count();
                let confidence = 0.60 + (0.08 * matched.min(5) as f64);
                drafts.push(FeatureDraft {
                    title: title.to_string(),
                    description: desc.to_string(),
                    tags: tags.iter().map(|s| s.to_string()).collect(),
                    confidence,
                });
            }
        }
    }

    // Fallback: if no features matched, create a single generic feature.
    if drafts.is_empty() {
        drafts.push(FeatureDraft {
            title: title_from_prompt(prompt),
            description: prompt.to_string(),
            tags: vec!["general".to_string()],
            confidence: 0.65,
        });
    }

    drafts.truncate(max);
    drafts
}

fn slugify(text: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\s]+").unwrap();
    let cleaned = re.replace_all(text, " ");
    cleaned
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn extract_title(prompt: &str) -> String {
    let first = prompt.split('.').next().unwrap_or(prompt);
    let first = first.split('\n').next().unwrap_or(first);
    let first = first.trim();
    if first.len() > 80 {
        format!("{}...", &first[..77])
    } else {
        first.to_string()
    }
}

fn title_from_prompt(prompt: &str) -> String {
    let first = prompt.split('.').next().unwrap_or(prompt);
    let first = first.split('\n').next().unwrap_or(first);
    let first = first.trim();
    if first.len() > 60 {
        format!("{}...", &first[..57])
    } else {
        first.to_string()
    }
}

fn extract_priority(prompt: &str) -> String {
    let lower = prompt.to_lowercase();
    if lower.contains("critical")
        || lower.contains("urgent")
        || lower.contains("p0")
        || lower.contains("blocker")
    {
        "critical".to_string()
    } else if lower.contains("high") || lower.contains("important") || lower.contains("p1") {
        "high".to_string()
    } else if lower.contains("low") || lower.contains("nice to have") || lower.contains("p3") {
        "low".to_string()
    } else {
        "medium".to_string()
    }
}

fn extract_stakeholders(prompt: &str) -> Vec<String> {
    let lower = prompt.to_lowercase();
    let mut out = vec![];
    if lower.contains("user") || lower.contains("customer") {
        out.push("end-user".to_string());
    }
    if lower.contains("admin") || lower.contains("operator") {
        out.push("admin".to_string());
    }
    if lower.contains("dev") || lower.contains("engineer") || lower.contains("developer") {
        out.push("developer".to_string());
    }
    if lower.contains("qa") || lower.contains("tester") {
        out.push("qa".to_string());
    }
    if lower.contains("designer") || lower.contains("ux") {
        out.push("designer".to_string());
    }
    out
}

fn extract_acceptance_criteria(prompt: &str) -> Vec<String> {
    let lower = prompt.to_lowercase();
    let mut out = vec![];
    if lower.contains("visible") || lower.contains("see") || lower.contains("display") {
        out.push("UI element is visible to the user".to_string());
    }
    if lower.contains("toggle") || lower.contains("switch") || lower.contains("enable") {
        out.push("User can toggle the feature on/off".to_string());
    }
    if lower.contains("save") || lower.contains("persist") || lower.contains("store") {
        out.push("Changes persist across sessions".to_string());
    }
    if lower.contains("mobile") || lower.contains("responsive") || lower.contains("screen") {
        out.push("Feature works on mobile and desktop".to_string());
    }
    if out.is_empty() {
        out.push("Feature satisfies the described user need".to_string());
    }
    out
}
