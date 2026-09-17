//! Evidence artifact and bundle handling for feature verification.
//!
//! This module provides HTTP handlers for:
//! - Loading evidence bundles from disk (`.agileplus/evidence/<feature_id>/bundle.json`)
//! - Serving evidence content and previews
//! - Generating new evidence bundles via `scripts/generate-evidence.sh`
//! - Exposing evidence gallery metadata as JSON for lightbox integration
//!
//! Evidence bundles contain test results, git history, PR links, and CI/CD artifacts
//! that collectively establish traceability and acceptance criteria compliance.

use std::fs;
use std::path::PathBuf;

use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::app_state::SharedState;
use crate::templates::{
    CiLinkView, EvidenceBundleView, FeatureEvidencePartial, GenerateEvidenceResponse,
    GitCommitView, PrLinkView,
};

// ── JSON Response Types ────────────────────────────────────────────────────

/// JSON response for GET /api/dashboard/features/{id}/evidence.json
/// Used by lightbox integrations to fetch artifact metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceGalleryJson {
    pub feature_id: String,
    pub artifacts: Vec<EvidenceArtifactJson>,
    pub generated_at: Option<String>,
}

/// Individual artifact metadata within an evidence gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceArtifactJson {
    pub id: String,
    pub type_: String,
    pub title: String,
    pub path: String,
    pub url: String,
    pub created_at: String,
}

// ── Utilities ──────────────────────────────────────────────────────────────

/// Minimal HTML entity escaping for embedding text content in HTML attributes/elements.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ── Disk Loaders ──────────────────────────────────────────────────────────

/// Load real evidence bundles from `.agileplus/evidence/<feature_id>/bundle.json`.
/// Returns a vector of parsed evidence views, or empty if the bundle does not exist.
pub fn load_evidence_bundles_from_disk(feature_id: &str) -> Vec<EvidenceBundleView> {
    let bundle_path = PathBuf::from(".agileplus")
        .join("evidence")
        .join(feature_id)
        .join("bundle.json");

    let content = match fs::read_to_string(&bundle_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let timestamp = val["timestamp"].as_str().unwrap_or("unknown").to_string();

    // Parse test_results
    let tr = &val["test_results"];
    let test_passed = tr["passed"].as_bool();
    let tests_passed_count = tr["passed_count"].as_u64().unwrap_or(0) as u32;
    let tests_failed_count = tr["failed_count"].as_u64().unwrap_or(0) as u32;
    let test_summary = tr["summary"].as_str().map(str::to_string);
    let test_output = tr["output_snippet"].as_str().map(str::to_string);

    // Parse git commits
    let git_commits: Vec<GitCommitView> = val["git_log"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|c| GitCommitView {
            short_hash: c["short_hash"].as_str().unwrap_or("").to_string(),
            subject: c["subject"].as_str().unwrap_or("").to_string(),
            date: c["date"].as_str().unwrap_or("").to_string(),
            author: c["author"].as_str().unwrap_or("").to_string(),
            url: c["url"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    // Parse PRs
    let pr_links: Vec<PrLinkView> = val["prs"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|p| PrLinkView {
            number: p["number"].as_u64().unwrap_or(0),
            title: p["title"].as_str().unwrap_or("").to_string(),
            url: p["url"].as_str().unwrap_or("").to_string(),
            state: p["state"].as_str().unwrap_or("").to_lowercase(),
            head_ref: p["headRefName"].as_str().unwrap_or("").to_string(),
            created_at: p["createdAt"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    // Parse CI links
    let ci_links: Vec<CiLinkView> = val["ci_links"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|c| CiLinkView {
            id: c["id"].as_i64().unwrap_or(0),
            title: c["title"].as_str().unwrap_or("").to_string(),
            status: c["status"].as_str().unwrap_or("").to_string(),
            conclusion: c["conclusion"].as_str().unwrap_or("pending").to_string(),
            url: c["url"].as_str().unwrap_or("").to_string(),
            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let commit_count = git_commits.len();
    let pr_count = pr_links.len();
    let status = if test_passed.unwrap_or(false) {
        "verified"
    } else {
        "generated"
    };

    vec![EvidenceBundleView {
        id: format!("bundle-{feature_id}-disk"),
        fr_id: format!("FR-{feature_id}"),
        evidence_type: "generated_bundle".into(),
        wp_id: "auto".into(),
        wp_title: format!("Evidence Bundle — {feature_id}"),
        artifact_path: bundle_path.display().to_string(),
        created_at: timestamp,
        artifact_ext: "json".into(),
        status: status.into(),
        content_preview: test_output,
        is_text_artifact: true,
        is_image_artifact: false,
        download_url: format!("/api/features/{feature_id}/evidence/bundle.json"),
        test_passed,
        tests_passed_count,
        tests_failed_count,
        test_summary,
        commit_count,
        pr_count,
        ci_links,
        git_commits,
        pr_links,
    }]
}

// ── HTTP Handlers ──────────────────────────────────────────────────────────

/// `GET /api/evidence/{feature_id}/{artifact_id}`
/// Serves raw evidence artifact content from `.agileplus/evidence/<feature_id>/<artifact_id>`.
/// HTML-escapes plaintext for safe browser rendering; validates artifact_id to prevent path traversal.
pub async fn evidence_content(
    State(_state): State<SharedState>,
    Path((feature_id, artifact_id)): Path<(i64, String)>,
) -> Response {
    // Serve from .agileplus/evidence/<feature_id>/<artifact_id>
    let base_path = PathBuf::from(".agileplus")
        .join("evidence")
        .join(feature_id.to_string());

    // Validate artifact_id to prevent path traversal attacks
    if artifact_id.contains("..") || artifact_id.starts_with('/') || artifact_id.contains('\0') {
        return Html("# Forbidden\n\nInvalid artifact ID.".to_string()).into_response();
    }

    let artifact_path = base_path.join(&artifact_id);

    // Ensure the resolved path is within the base directory (security check)
    if !artifact_path.starts_with(&base_path) {
        return Html("# Forbidden\n\nPath traversal detected.".to_string()).into_response();
    }

    if let Ok(content) = fs::read_to_string(&artifact_path) {
        let escaped = html_escape(&content);
        return Html(format!(
            "<pre class='text-xs font-mono text-zinc-300 whitespace-pre-wrap'>{escaped}</pre>",
        ))
        .into_response();
    }

    Html(format!(
        "# Evidence Bundle {feature_id}\n\n## Artifact ID: {artifact_id}\n\nNo artifact found at expected path."
    ))
    .into_response()
}

/// `GET /api/evidence/{feature_id}/{artifact_id}/preview`
/// Serves a brief HTML preview of an evidence artifact (for lightbox/modal display).
pub async fn evidence_preview(
    State(_state): State<SharedState>,
    Path((feature_id, artifact_id)): Path<(i64, String)>,
) -> Response {
    let artifact_path = PathBuf::from(".agileplus")
        .join("evidence")
        .join(feature_id.to_string())
        .join(&artifact_id);

    let text = fs::read_to_string(&artifact_path)
        .unwrap_or_else(|_| format!("No preview — artifact not found: {artifact_id}"));
    let escaped = html_escape(&text);
    let preview = format!(
        "<div class='p-3 rounded bg-zinc-800 border border-zinc-700'>\
         <pre class='text-xs font-mono text-zinc-300 max-h-48 overflow-y-auto'>{escaped}</pre>\
         </div>"
    );
    Html(preview).into_response()
}

/// `GET /api/features/{id}/evidence`
/// Returns the evidence gallery partial (HTML template) for the feature detail page.
pub async fn feature_evidence_list(
    State(_state): State<SharedState>,
    Path(feature_id): Path<String>,
) -> Response {
    let bundles = load_evidence_bundles_from_disk(&feature_id);
    let tmpl = FeatureEvidencePartial {
        evidence_bundles: bundles,
    };
    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, template = "evidence", "askama template render failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `POST /api/features/{id}/evidence/generate`
/// Spawns `scripts/generate-evidence.sh <feature-id>` asynchronously and
/// returns a JSON status response immediately. The actual generation happens in the background.
pub async fn feature_evidence_generate(
    State(_state): State<SharedState>,
    Path(feature_id): Path<String>,
) -> Response {
    // Locate the script relative to the process working directory.
    let script = PathBuf::from("scripts").join("generate-evidence.sh");

    if !script.exists() {
        return axum::Json(GenerateEvidenceResponse {
            feature_id: feature_id.clone(),
            bundle_path: String::new(),
            status: "error".into(),
            message:
                "generate-evidence.sh not found — ensure the server is started from the repo root"
                    .into(),
        })
        .into_response();
    }

    let bundle_path = format!(".agileplus/evidence/{feature_id}/bundle.json");
    let fid = feature_id.clone();
    let bp = bundle_path.clone();

    // Spawn async so the HTTP response returns immediately.
    tokio::spawn(async move {
        let out = tokio::process::Command::new("bash")
            .arg(&script)
            .arg(&fid)
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => {
                tracing::info!(feature_id = %fid, bundle_path = %bp, "evidence bundle generated");
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::warn!(feature_id = %fid, stderr = %stderr, "evidence generation failed");
            }
            Err(e) => {
                tracing::error!(feature_id = %fid, error = %e, "failed to spawn generate-evidence.sh");
            }
        }
    });

    axum::Json(GenerateEvidenceResponse {
        feature_id,
        bundle_path,
        status: "started".into(),
        message: "Evidence generation started — poll GET /api/features/{id}/evidence for results"
            .into(),
    })
    .into_response()
}

/// `GET /api/dashboard/features/{id}/evidence.json`
/// Returns evidence gallery metadata as JSON for lightbox/modal integration.
/// Extracts artifact metadata from loaded bundles and serializes as a structured JSON response.
pub async fn feature_evidence_json(
    State(_state): State<SharedState>,
    Path(feature_id): Path<String>,
) -> impl IntoResponse {
    let bundles = load_evidence_bundles_from_disk(&feature_id);

    // Extract artifacts from bundles for gallery JSON response
    let artifacts: Vec<EvidenceArtifactJson> = bundles
        .iter()
        .map(|bundle| EvidenceArtifactJson {
            id: bundle.id.clone(),
            type_: bundle.evidence_type.clone(),
            title: bundle.wp_title.clone(),
            path: bundle.artifact_path.clone(),
            url: format!("/api/evidence/{}/{}/preview", feature_id, bundle.id),
            created_at: bundle.created_at.clone(),
        })
        .collect();

    let generated_at = bundles.first().map(|b| b.created_at.clone());

    axum::Json(EvidenceGalleryJson {
        feature_id,
        artifacts,
        generated_at,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape_ampersand() {
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_html_escape_angle_brackets() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn test_html_escape_quotes() {
        assert_eq!(html_escape("say \"hello\""), "say &quot;hello&quot;");
        assert_eq!(html_escape("it's"), "it&#39;s");
    }

    #[test]
    fn test_html_escape_no_op_on_plain_text() {
        let plain = "Hello, world!";
        assert_eq!(html_escape(plain), plain);
    }
}
