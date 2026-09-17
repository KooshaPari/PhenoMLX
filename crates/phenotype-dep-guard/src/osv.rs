// SPDX-License-Identifier: MIT OR Apache-2.0
//! Async client for OSV.dev (`https://api.osv.dev`).
//!
//! The production endpoint requires no auth. For tests, point
//! `OsvClient::with_endpoint` at a `wiremock` server.

use std::time::Duration;

use reqwest::Client;

use crate::dependency::Dependency;
use crate::error::{Error, Result};
use crate::vulnerability::{OsvPackage, OsvQuery, OsvResponse, OsvVuln, Vulnerability};

const DEFAULT_ENDPOINT: &str = "https://api.osv.dev";
const QUERY_PATH: &str = "/v1/query";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const BATCH_SIZE: usize = 16;

/// Async OSV.dev client.
#[derive(Debug, Clone)]
pub struct OsvClient {
    endpoint: String,
    http: Client,
}

impl Default for OsvClient {
    fn default() -> Self {
        Self::new().expect("OSV client should build with default config")
    }
}

impl OsvClient {
    /// Build a client targeting the real OSV.dev endpoint.
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent(concat!("phenotype-dep-guard/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            http,
        })
    }

    /// Build a client targeting a custom endpoint (used in tests).
    pub fn with_endpoint(endpoint: impl Into<String>) -> Result<Self> {
        let mut s = Self::new()?;
        s.endpoint = endpoint.into();
        Ok(s)
    }

    /// Query OSV for a single dependency's known vulnerabilities.
    pub async fn query(&self, dep: &Dependency) -> Result<Vec<Vulnerability>> {
        let url = format!("{}{}", self.endpoint, QUERY_PATH);
        let body = OsvQuery {
            version: "1.6.0",
            package: OsvPackage {
                name: &dep.name,
                ecosystem: dep.ecosystem.as_osv_str(),
            },
        };
        let resp = self.http.post(&url).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::OsvStatus {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: OsvResponse = resp.json().await?;
        let vulns = parsed
            .vulns
            .into_iter()
            .map(|v: OsvVuln| v.into_vuln(&dep.version))
            .collect();
        Ok(vulns)
    }

    /// Query OSV for many dependencies, returning findings keyed by
    /// dependency name+version.
    pub async fn query_batch(
        &self,
        deps: &[Dependency],
    ) -> Result<Vec<(Dependency, Vec<Vulnerability>)>> {
        let mut out = Vec::with_capacity(deps.len());
        for chunk in deps.chunks(BATCH_SIZE) {
            // Fan out within the chunk concurrently; OSV is fine with
            // parallel requests and a small chunk keeps memory bounded.
            let mut futs = Vec::with_capacity(chunk.len());
            for d in chunk {
                let fut = self.query(d);
                futs.push(fut);
            }
            // Sequential within chunk keeps error attribution simple.
            for (d, f) in chunk.iter().zip(futs.into_iter()) {
                out.push((d.clone(), f.await?));
            }
        }
        Ok(out)
    }
}
