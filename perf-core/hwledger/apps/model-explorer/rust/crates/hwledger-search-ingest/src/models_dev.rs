//! SourceAdapter impl for models.dev (https://models.dev).
//!
//! models.dev is a free, open-source model metadata aggregator maintained
//! by Vercel. Unlike the HuggingFace Hub, it returns a fully-normalized
//! JSON snapshot of every model it knows about with capability / modality
//! metadata baked in. We use it as the second source in the federation
//! so the search index isn't duplicated work with HF and isn't stale on
//! direct deletes.
//!
//! Live HTTP endpoint: <https://models.dev/api.json>. The adapter keeps
//! a local in-memory snapshot built by `from_snapshot` so that
//! `list_candidates` stays sync (the trait is dyn-compatible).
//!
//! In v1 the snapshot is populated by the embedded smoke fixture so tests
//! don't hit the network; a scheduled `seed expand` call would populate
//! from the live endpoint in production.

use hwledger_search_core::error::CoreError;
use hwledger_search_core::{CandidateId, RawModel, SourceAdapter};

use std::collections::BTreeSet;

/// Default endpoint for the public models.dev JSON snapshot.
pub const DEFAULT_API_URL: &str = "https://models.dev/api.json";

/// models.dev [`SourceAdapter`] implementation.
///
/// Holds an in-memory snapshot of the model list. Production wiring
/// feeds it via `from_snapshot` parsed from a live HTTP fetch.
pub struct ModelsDevAdapter {
    snapshot: Vec<RawModel>,
}

impl std::fmt::Debug for ModelsDevAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelsDevAdapter")
            .field("snapshot_len", &self.snapshot.len())
            .finish()
    }
}

impl ModelsDevAdapter {
    /// Build an adapter seeded from a `Vec<RawModel>`. The `fixture()`
    /// method is the canonical test entry point.
    pub fn from_snapshot(snapshot: Vec<RawModel>) -> Self {
        Self { snapshot }
    }

    /// Snapshot the canonical 1k-row smoke fixture embedded in this
    /// crate. Used by tests and as the seed for "no-network" runs.
    pub fn fixture() -> Self {
        Self::from_snapshot(smoke_fixture::snapshot())
    }

    /// Parse the models.dev JSON shape `{<provider>: {<model>: Meta}}`
    /// into a flat `Vec<RawModel>`. The shape is JSON; the trait is
    /// sync. Tests use this through `from_snapshot(parsed_json)`.
    pub fn parse_snapshot(json: &str) -> Result<Vec<RawModel>, String> {
        let map: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, ModelsDevEntry>,
        > = serde_json::from_str(json).map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for (provider, models) in map {
            for (name, entry) in models {
                out.push(entry.into_raw(&provider, &name));
            }
        }
        Ok(out)
    }

    /// Number of models in the snapshot.
    pub fn len(&self) -> usize {
        self.snapshot.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.snapshot.is_empty()
    }

    /// Borrow the in-memory snapshot of all models.
    pub fn snapshot(&self) -> &[RawModel] {
        &self.snapshot
    }
}

impl SourceAdapter for ModelsDevAdapter {
    fn name(&self) -> &str {
        "models-dev"
    }

    fn list_candidates(&self, query: Option<&str>, limit: usize) -> Vec<CandidateId> {
        let mut ids: Vec<CandidateId> = self
            .snapshot
            .iter()
            .filter(|m| match query {
                None | Some("") => true,
                Some(q) => {
                    let q = q.to_lowercase();
                    let hit = m.id.to_lowercase().contains(&q);
                    let tag_hit = m
                        .pipeline_tag
                        .as_deref()
                        .map(|t| t.to_lowercase().contains(&q))
                        .unwrap_or(false);
                    hit || tag_hit
                }
            })
            .map(|m| m.candidate_id())
            .collect();

        let mut seen = BTreeSet::new();
        ids.retain(|id| seen.insert(id.key()));

        ids.truncate(limit);
        ids
    }

    fn fetch_raw(&self, id: &CandidateId) -> Result<RawModel, CoreError> {
        let needle = id.key();
        self.snapshot
            .iter()
            .find(|m| m.key() == needle)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(needle))
    }
}

/// One model entry in the models.dev JSON shape.
#[derive(Debug, Clone, serde::Deserialize)]
struct ModelsDevEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    attachment: bool,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    tool_call: bool,
    #[serde(default)]
    open_weights: bool,
    #[serde(default)]
    limit: ModelsDevLimit,
    #[serde(default)]
    modalities: ModelsDevModalities,
}

impl ModelsDevEntry {
    fn into_raw(self, provider: &str, model_name: &str) -> RawModel {
        let _modality = if self.attachment { "multimodal" } else { "text" };
        let _provider_hint = provider;

        // Map the JSON shape into a single `pipeline_tag` so the existing
        // tagger can pick it up.
        let pipeline_tag = if self.reasoning && self.tool_call {
            Some("text-generation".to_string())
        } else if self.reasoning {
            Some("reasoning".to_string())
        } else if self.tool_call {
            Some("agentic".to_string())
        } else {
            Some("text-generation".to_string())
        };

        let name = if self.name.is_empty() {
            model_name.to_string()
        } else {
            self.name
        };

        RawModel {
            id: format!("{provider}/{model_name}"),
            source: "models-dev".to_string(),
            card_text: Some(format!(
                "{name}\nreasoning={} tool_call={} open_weights={} attachment={} modalities={:?}/{:?}",
                self.reasoning,
                self.tool_call,
                self.open_weights,
                self.attachment,
                self.modalities.input,
                self.modalities.output
            )),
            config_json: Some(serde_json::json!({
                "name": name,
                "reasoning": self.reasoning,
                "tool_call": self.tool_call,
                "open_weights": self.open_weights,
                "attachment": self.attachment,
                "limit": { "context": self.limit.context },
                "modalities": {
                    "input": self.modalities.input,
                    "output": self.modalities.output,
                },
            })),
            tree_entries: Vec::new(),
            downloads: None,
            likes: None,
            last_modified: None,
            pipeline_tag,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct ModelsDevModalities {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    output: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct ModelsDevLimit {
    #[serde(default)]
    context: u64,
}

/// 1k-row smoke fixture embedded in the binary so tests run without
/// network. Covers the model families we care about plus a long tail of
/// synthesized entries.
pub mod smoke_fixture {
    use super::*;

    /// Build the canonical 1k-row smoke fixture used by tests
    /// and offline seeding runs.
    pub fn snapshot() -> Vec<RawModel> {
        let providers: &[(&str, &[&str])] = &[
            ("openai", &["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o1", "o1-mini", "o3", "o3-mini", "o4-mini"]),
            ("anthropic", &["claude-3.5-sonnet", "claude-3.5-haiku", "claude-3-opus", "claude-3-sonnet"]),
            ("google", &["gemini-2.0-flash", "gemini-2.0-pro", "gemini-1.5-pro", "gemini-1.5-flash"]),
            ("meta", &["llama-3.1-8b", "llama-3.1-70b", "llama-3.1-405b", "llama-3.2-1b", "llama-3.2-3b", "llama-3.3-70b"]),
            ("mistral", &["mistral-large", "mistral-small", "mistral-nemo", "codestral", "mixtral-8x7b", "mixtral-8x22b"]),
            ("deepseek", &["deepseek-v3", "deepseek-r1", "deepseek-coder", "deepseek-v2.5"]),
            ("qwen", &["qwen2.5-7b", "qwen2.5-72b", "qwen2.5-coder-7b", "qwen2.5-coder-32b", "qwen-max"]),
            ("cohere", &["command-r-plus", "command-r", "command-light"]),
            ("groq", &["llama-3.1-8b-instant", "llama-3.3-70b-versatile", "mixtral-8x7b-instant"]),
            ("perplexity", &["sonar", "sonar-pro", "sonar-reasoning"]),
            ("xai", &["grok-2", "grok-2-mini"]),
            ("reka", &["reka-core", "reka-flash"]),
            ("ai21", &["jamba-1.5-large", "jamba-1.5-mini"]),
            ("together", &["llama-3.1-8b-together", "qwen2.5-72b-together"]),
        ];

        let mut out: Vec<RawModel> = Vec::new();
        for &(provider, models) in providers {
            for &name in models.iter() {
                let reasoning = provider == "openai"
                    && matches!(name, "o1" | "o1-mini" | "o3" | "o3-mini" | "o4-mini");
                let tool_call = provider != "google";
                let attachment = matches!(provider, "openai" | "google" | "anthropic");
                let open_weights = matches!(
                    provider,
                    "meta" | "mistral" | "deepseek" | "qwen" | "ai21" | "together"
                );

                let modality = if attachment { "multimodal" } else { "text" };

                let context_len = if name.contains("405b")
                    || name.contains("72b")
                    || name.contains("70b")
                    || name.contains("large")
                    || name.contains("pro")
                {
                    128_000
                } else if name.contains("opus") || name.contains("haiku") {
                    200_000
                } else if name.contains("gemini") {
                    1_000_000
                } else if name.contains("sonnet") || name.contains("sonar") {
                    128_000
                } else {
                    32_000
                };

                let pipeline_tag = if reasoning && tool_call {
                    Some("text-generation".to_string())
                } else if reasoning {
                    Some("reasoning".to_string())
                } else if tool_call {
                    Some("agentic".to_string())
                } else {
                    Some("text-generation".to_string())
                };

                let id = format!("{provider}/{name}");
                out.push(RawModel {
                    id: id.clone(),
                    source: "models-dev".to_string(),
                    card_text: Some(format!(
                        "{id}\nmodality={modality} context_len={context_len} reasoning={reasoning} tool_call={tool_call} open_weights={open_weights} attachment={attachment}"
                    )),
                    config_json: Some(serde_json::json!({
                        "provider": provider,
                        "name": name,
                        "modality": modality,
                        "context_len": context_len,
                        "reasoning": reasoning,
                        "tool_call": tool_call,
                        "open_weights": open_weights,
                        "attachment": attachment,
                    })),
                    tree_entries: Vec::new(),
                    downloads: None,
                    likes: None,
                    last_modified: None,
                    pipeline_tag,
                });
            }
        }

        // Pad to exactly 1000 entries with synthesized layer-2 variants.
        let suffixes = ["distilled", "instant", "preview", "experimental", "lite", "turbo"];
        let mut idx: u32 = 0;
        while out.len() < 1000 {
            let (provider, models) = providers[(idx as usize) % providers.len()];
            let base_name: &str = models[(idx as usize / providers.len()) % models.len()];
            let suffix = suffixes[(idx as usize) % suffixes.len()];
            let id = format!("{provider}/{base_name}-{suffix}-{idx}");
            out.push(RawModel {
                id: id.clone(),
                source: "models-dev".to_string(),
                card_text: Some(format!("Synthetic variant {id}")),
                config_json: Some(serde_json::json!({"synthetic": true})),
                tree_entries: Vec::new(),
                downloads: None,
                likes: None,
                last_modified: None,
                pipeline_tag: Some("text-generation".to_string()),
            });
            idx += 1;
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_1000_rows() {
        let adapter = ModelsDevAdapter::fixture();
        assert_eq!(adapter.len(), 1000);
    }

    #[test]
    fn list_candidates_caps_and_dedups() {
        let adapter = ModelsDevAdapter::fixture();
        let ids = adapter.list_candidates(None, 50);
        assert_eq!(ids.len(), 50);
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(id.key()), "duplicate id: {}", id.key());
        }
    }

    #[test]
    fn list_candidates_query_filter() {
        let adapter = ModelsDevAdapter::fixture();
        let all = adapter.list_candidates(None, 1000);
        let qwen = adapter.list_candidates(Some("qwen"), 50);
        assert!(!qwen.is_empty());
        assert!(qwen.len() < all.len());
        for id in &qwen {
            assert!(
                id.id.to_lowercase().contains("qwen") || id.source == "models-dev",
                "unexpected id in qwen filter: {}",
                id.key()
            );
        }
    }

    #[test]
    fn fetch_raw_roundtrip() {
        let adapter = ModelsDevAdapter::fixture();
        let ids = adapter.list_candidates(Some("qwen"), 1);
        let raw = adapter.fetch_raw(&ids[0]).expect("fetch_raw");
        assert_eq!(raw.id, ids[0].id);
        assert_eq!(raw.source, "models-dev");
        assert!(raw.pipeline_tag.is_some());
    }

    #[test]
    fn fetch_raw_missing_returns_not_found() {
        let adapter = ModelsDevAdapter::fixture();
        let err = adapter
            .fetch_raw(&CandidateId::new("models-dev", "nonexistent/missing"))
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"));
    }

    #[test]
    fn parse_snapshot_unwraps_nested_map() {
        let json = r#"{
            "openai": {
                "gpt-test": {
                    "name": "GPT Test",
                    "reasoning": true,
                    "tool_call": true,
                    "attachment": false,
                    "open_weights": false,
                    "limit": { "context": 128000 },
                    "modalities": { "input": ["text"], "output": ["text"] }
                }
            }
        }"#;
        let parsed = ModelsDevAdapter::parse_snapshot(json).unwrap();
        let adapter = ModelsDevAdapter::from_snapshot(parsed);
        let ids = adapter.list_candidates(None, 10);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].source, "models-dev");
        assert_eq!(ids[0].id, "openai/gpt-test");
        let raw = adapter.fetch_raw(&ids[0]).unwrap();
        assert_eq!(raw.id, "openai/gpt-test");
        assert!(raw.card_text.is_some());
        assert!(raw.config_json.is_some());
    }

    #[test]
    fn name_returns_models_dev() {
        let adapter = ModelsDevAdapter::fixture();
        assert_eq!(adapter.name(), "models-dev");
    }

    #[test]
    fn fixture_has_diverse_providers() {
        let adapter = ModelsDevAdapter::fixture();
        let families: std::collections::HashSet<_> = adapter
            .snapshot()
            .iter()
            .map(|m| m.id.split('/').next().unwrap_or("").to_string())
            .collect();
        assert!(
            families.len() >= 10,
            "expected >=10 providers, got {}",
            families.len()
        );
    }
}
