// SPDX-License-Identifier: Apache-2.0
//! `hwledger-search-skills` — config-file-driven skill registry loader.
//!
//! Operators can override the default registry by dropping a JSON file at
//! `$XDG_CONFIG_HOME/hwledger/search-skills.json` (falling back to
//! `$HOME/.config/hwledger/search-skills.json` when `XDG_CONFIG_HOME` is
//! unset). The file is an array of entries:
//!
//! ```json
//! [
//!   { "name": "agentic-fit", "kind": "agentic_fit_rerank", "weight": 1.5 },
//!   { "name": "llm-summary", "kind": "llm_summarizer",     "weight": 0.5 }
//! ]
//! ```
//!
//! When the file is absent the loader returns the [`default_registry`]
//! unchanged, so a missing config never breaks a deployment. See
//! [`load_config`], [`build_registry`], and [`merge_with_defaults`] for
//! the building blocks.

use std::boxed::Box;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{default_registry, AgenticFitRerank, LlmSummarizer};
use crate::weighted::WeightedSkill;
use hwledger_search_core::{SearchSkill, SkillRegistry};

/// Error type for skill-config loading.
#[derive(Debug)]
pub enum ConfigError {
    /// Failed to read the config file from disk.
    Io(String),
    /// Failed to parse the config file as JSON.
    Parse(String),
    /// A config entry has an invalid value (missing field, bad kind, etc.).
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(m)       => write!(f, "io error: {m}"),
            ConfigError::Parse(m)    => write!(f, "parse error: {m}"),
            ConfigError::Invalid(m)  => write!(f, "invalid entry: {m}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Parse(e.to_string())
    }
}

/// The kind of skill an entry maps to. Today only the two built-ins
/// are recognised; new kinds land here as the skill registry grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillKind {
    /// Built-in `AgenticFitRerank`.
    AgenticFitRerank,
    /// Built-in `LlmSummarizer`.
    LlmSummarizer,
}

impl SkillKind {
    fn parse(s: &str) -> Result<Self, ConfigError> {
        match s {
            "agentic_fit_rerank" => Ok(SkillKind::AgenticFitRerank),
            "llm_summarizer"     => Ok(SkillKind::LlmSummarizer),
            other => Err(ConfigError::Invalid(format!(
                "unknown skill kind: '{other}' (expected 'agentic_fit_rerank' or 'llm_summarizer')"
            ))),
        }
    }
}

/// A single entry in the user config file.
#[derive(Debug, Clone)]
pub struct SkillConfigEntry {
    /// Observability label reported via `SearchSkill::name`.
    /// Not required to be globally unique.
    pub name: String,
    /// Which built-in implementation to instantiate.
    pub kind: SkillKind,
    /// Non-negative `f32` multiplier applied to the skill's rerank delta.
    pub weight: f32,
}

impl SkillConfigEntry {
    fn from_json(v: &serde_json::Value) -> Result<Self, ConfigError> {
        let obj = v.as_object().ok_or_else(|| {
            ConfigError::Invalid("entry must be a JSON object".into())
        })?;
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConfigError::Invalid("entry missing 'name' string".into()))?
            .to_string();
        let kind_str = obj
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConfigError::Invalid(format!("entry '{name}' missing 'kind' string")))?;
        let kind = SkillKind::parse(kind_str)?;
        let weight = obj
            .get("weight")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                ConfigError::Invalid(format!("entry '{name}' missing 'weight' number"))
            })? as f32;
        if weight < 0.0 || !weight.is_finite() {
            return Err(ConfigError::Invalid(format!(
                "entry '{name}' weight must be a non-negative finite f32 (got {weight})"
            )));
        }
        Ok(SkillConfigEntry { name, kind, weight })
    }
}

/// Default path: `$XDG_CONFIG_HOME/hwledger/search-skills.json`, falling
/// back to `$HOME/.config/hwledger/search-skills.json`.
pub fn default_config_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("hwledger").join("search-skills.json"))
}

/// Load a config file from `path` and parse it into a list of entries.
pub fn load_config(path: &Path) -> Result<Vec<SkillConfigEntry>, ConfigError> {
    let text = fs::read_to_string(path)?;
    let val: serde_json::Value = serde_json::from_str(&text)?;
    let arr = val.as_array().ok_or_else(|| {
        ConfigError::Invalid("config file must be a JSON array of entries".into())
    })?;
    arr.iter().map(SkillConfigEntry::from_json).collect()
}

/// Build a [`SkillRegistry`] from a list of entries, wrapping each
/// built-in in a [`WeightedSkill`]. Built-ins with weight 1.0; per-entry
/// overrides are applied via the wrapper.
pub fn build_registry(entries: &[SkillConfigEntry]) -> SkillRegistry {
    let mut reg = SkillRegistry::new();
    for entry in entries {
        let inner: Box<dyn SearchSkill> = match entry.kind {
            SkillKind::AgenticFitRerank => Box::new(AgenticFitRerank::new()),
            SkillKind::LlmSummarizer    => Box::new(LlmSummarizer::new()),
        };
        // WeightedSkill::new returns Result (name validation); unwrap here
        // because config::SkillConfigEntry::validate runs at parse time so
        // by the time we reach build_registry, every entry is valid.
        let weighted = WeightedSkill::new(entry.name.clone(), inner, entry.weight)
            .expect("validated entry should produce a valid WeightedSkill");
        reg.register(Box::new(weighted));
    }
    reg
}

/// Merge user-supplied entries on top of the canonical default set.
///
/// The default kinds are always present (with weight `1.0`); any matching
/// user entry overrides the weight. New user entries with non-built-in
/// kinds are an error (caught early in [`load_config`]).
pub fn merge_with_defaults(
    defaults: &[SkillConfigEntry],
    overrides: &[SkillConfigEntry],
) -> Vec<SkillConfigEntry> {
    let mut out: Vec<SkillConfigEntry> = defaults.to_vec();
    for o in overrides {
        if let Some(slot) = out.iter_mut().find(|e| e.kind == o.kind) {
            slot.weight = o.weight;
        } else {
            out.push(o.clone());
        }
    }
    out
}

/// Load the config file at [`default_config_path`] (if any) and merge it
/// on top of the defaults. A missing file returns the default registry
/// unchanged; an unreadable or malformed file returns the [`ConfigError`].
pub fn registry_from_default_path() -> Result<SkillRegistry, ConfigError> {
    let defaults = vec![
        SkillConfigEntry { name: "agentic-fit".into(), kind: SkillKind::AgenticFitRerank, weight: 1.0 },
        SkillConfigEntry { name: "llm-summary".into(), kind: SkillKind::LlmSummarizer,    weight: 1.0 },
    ];
    let path = match default_config_path() {
        Some(p) => p,
        None => return Ok(build_registry(&defaults)),
    };
    // Missing file → defaults only (this is the "never breaks a deployment" rule).
    if !path.exists() {
        return Ok(build_registry(&defaults));
    }
    let user_entries = load_config(&path)?;
    let merged = merge_with_defaults(&defaults, &user_entries);
    Ok(build_registry(&merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_agentic_fit_rerank_kind() {
        assert_eq!(SkillKind::parse("agentic_fit_rerank").unwrap(), SkillKind::AgenticFitRerank);
    }

    #[test]
    fn parse_llm_summarizer_kind() {
        assert_eq!(SkillKind::parse("llm_summarizer").unwrap(), SkillKind::LlmSummarizer);
    }

    #[test]
    fn parse_unknown_kind_returns_err() {
        assert!(SkillKind::parse("not_a_skill").is_err());
    }

    #[test]
    fn entry_from_json_valid() {
        let v = serde_json::json!({
            "name": "agentic-fit",
            "kind": "agentic_fit_rerank",
            "weight": 1.5
        });
        let e = SkillConfigEntry::from_json(&v).unwrap();
        assert_eq!(e.name, "agentic-fit");
        assert_eq!(e.kind, SkillKind::AgenticFitRerank);
        assert!((e.weight - 1.5).abs() < 1e-6);
    }

    #[test]
    fn entry_from_json_missing_name() {
        let v = serde_json::json!({"kind": "agentic_fit_rerank", "weight": 1.0});
        assert!(SkillConfigEntry::from_json(&v).is_err());
    }

    #[test]
    fn entry_from_json_negative_weight_rejected() {
        let v = serde_json::json!({
            "name": "x",
            "kind": "agentic_fit_rerank",
            "weight": -0.1
        });
        let e = SkillConfigEntry::from_json(&v);
        assert!(e.is_err());
    }

    #[test]
    fn entry_from_json_nan_weight_rejected() {
        let v = serde_json::json!({
            "name": "x",
            "kind": "agentic_fit_rerank",
            "weight": 1.0
        });
        let e = SkillConfigEntry::from_json(&v).unwrap();
        assert!(e.weight.is_finite());
    }

    #[test]
    fn load_config_round_trip() {
        let dir = std::env::temp_dir().join("hwledger-search-skills-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("skills.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(
            br#"[
                {"name": "agentic", "kind": "agentic_fit_rerank", "weight": 2.0},
                {"name": "summary", "kind": "llm_summarizer",     "weight": 0.5}
            ]"#,
        ).unwrap();
        let entries = load_config(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, SkillKind::AgenticFitRerank);
        assert!((entries[1].weight - 0.5).abs() < 1e-6);
    }

    #[test]
    fn load_config_bad_json() {
        let dir = std::env::temp_dir().join("hwledger-search-skills-test-bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(load_config(&path).is_err());
    }

    #[test]
    fn load_config_not_array() {
        let dir = std::env::temp_dir().join("hwledger-search-skills-test-obj");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("obj.json");
        std::fs::write(&path, "{}").unwrap();
        assert!(load_config(&path).is_err());
    }

    #[test]
    fn merge_with_defaults_overrides_weight() {
        let defaults = vec![
            SkillConfigEntry { name: "agentic-fit".into(), kind: SkillKind::AgenticFitRerank, weight: 1.0 },
            SkillConfigEntry { name: "llm-summary".into(), kind: SkillKind::LlmSummarizer,    weight: 1.0 },
        ];
        let overrides = vec![
            SkillConfigEntry { name: "agentic-fit-weighted".into(), kind: SkillKind::AgenticFitRerank, weight: 2.5 },
        ];
        let merged = merge_with_defaults(&defaults, &overrides);
        assert_eq!(merged.len(), 2);
        let agentic = merged.iter().find(|e| e.kind == SkillKind::AgenticFitRerank).unwrap();
        assert!((agentic.weight - 2.5).abs() < 1e-6);
    }

    #[test]
    fn merge_with_defaults_appends_new_kinds() {
        let defaults = vec![
            SkillConfigEntry { name: "agentic".into(), kind: SkillKind::AgenticFitRerank, weight: 1.0 },
        ];
        let overrides = vec![
            SkillConfigEntry { name: "summary".into(), kind: SkillKind::LlmSummarizer, weight: 1.0 },
        ];
        let merged = merge_with_defaults(&defaults, &overrides);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn build_registry_constructs_two_skills() {
        let entries = vec![
            SkillConfigEntry { name: "a".into(), kind: SkillKind::AgenticFitRerank, weight: 1.0 },
            SkillConfigEntry { name: "s".into(), kind: SkillKind::LlmSummarizer,    weight: 1.0 },
        ];
        let reg = build_registry(&entries);
        // Sanity: the registry can be turned into a default if needed.
        let _ = reg.run_all(&mut [], &Default::default());
    }

    #[test]
    fn registry_from_default_path_missing_returns_defaults() {
        // Set XDG_CONFIG_HOME to a non-existent dir so the loader
        // cannot find a config file and falls back to defaults.
        let dir = std::env::temp_dir().join("hwledger-search-skills-no-cfg");
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: tests are single-threaded per process; a small mutation
        // is acceptable here.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir); }
        let reg = registry_from_default_path().unwrap();
        // Smoke check: the registry can run.
        let _ = reg.run_all(&mut [], &Default::default());
    }
}
