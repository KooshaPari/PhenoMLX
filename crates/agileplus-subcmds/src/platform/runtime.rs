//! Resolved local listener endpoints used by platform diagnostics.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::platform::health::DEFAULT_API_URL;

const RUNTIME_PORTS_FILE: &str = ".agileplus/runtime/local-ports.env";

/// A coherent API base and health endpoint pair for the local platform runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedRuntime {
    api_base: String,
    health_url: String,
}

impl ResolvedRuntime {
    fn new(api_base: impl Into<String>, health_url: impl Into<String>) -> Result<Self> {
        let api_base = api_base.into().trim_end_matches('/').to_owned();
        let health_url = health_url.into();
        let expected_health_url = format!("{api_base}/health");
        if api_base.is_empty() || health_url != expected_health_url {
            return Err(anyhow!(
                "runtime API base and health URL must form one endpoint pair"
            ));
        }
        Ok(Self {
            api_base,
            health_url,
        })
    }

    pub(crate) fn from_api_base(api_base: impl Into<String>) -> Result<Self> {
        let api_base = api_base.into();
        let health_url = format!("{}/health", api_base.trim_end_matches('/'));
        Self::new(api_base, health_url)
    }

    /// Load the endpoint pair from the process environment, persisted runtime file, or default.
    pub(crate) fn load() -> Result<Self> {
        let environment_base = std::env::var("AGILEPLUS_API_URL").ok();
        Self::load_from_sources(
            environment_base.as_deref(),
            runtime_file_from_repo_root().as_deref(),
        )
    }

    pub(crate) fn load_from_sources(
        environment_base: Option<&str>,
        runtime_file: Option<&Path>,
    ) -> Result<Self> {
        if let Some(api_base) = environment_base.filter(|value| !value.trim().is_empty()) {
            return Self::from_api_base(api_base);
        }

        if let Some(runtime_file) = runtime_file.filter(|path| path.is_file()) {
            let values = parse_runtime_file(runtime_file)?;
            if let Some(api_base) = values.get("AGILEPLUS_API_URL") {
                if let Some(health_url) = values.get("AGILEPLUS_API_HEALTH_URL") {
                    return Self::new(api_base, health_url);
                }
                return Self::from_api_base(api_base);
            }
        }

        Self::from_api_base(DEFAULT_API_URL)
    }

    pub(crate) fn health_url(&self) -> &str {
        &self.health_url
    }
}

fn runtime_file_from_repo_root() -> Option<PathBuf> {
    let root = std::env::var("AGILEPLUS_ROOT")
        .ok()
        .map(PathBuf::from)
        .or_else(crate::platform::workspace::find_agileplus_root_from_walk)?;
    Some(root.join(RUNTIME_PORTS_FILE))
}

fn parse_runtime_file(path: &Path) -> Result<BTreeMap<String, String>> {
    let contents = fs::read_to_string(path)?;
    Ok(contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect())
}
