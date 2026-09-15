use base64::Engine;
use serde::Deserialize;

use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::helpers::{evals_python, evals_root, load_current_cells, truncate};

#[derive(Debug, Deserialize)]
pub struct TracesQuery {
    #[serde(default = "default_limit")]
    pub limit: String,
}

fn default_limit() -> String {
    "50".into()
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    #[serde(default = "default_max_cells")]
    pub max_cells: usize,
}

fn default_max_cells() -> usize {
    40
}

#[derive(Debug, Deserialize)]
pub struct EvaluatorsQuery {
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_limit")]
    pub limit: String,
}

fn default_action() -> String {
    "judge".into()
}

fn langfuse_enabled() -> bool {
    let pub_key = std::env::var("LANGFUSE_PUBLIC_KEY").unwrap_or_default();
    let sec_key = std::env::var("LANGFUSE_SECRET_KEY").unwrap_or_default();
    !pub_key.trim().is_empty() && !sec_key.trim().is_empty()
}

fn langfuse_base() -> String {
    for key in &["LANGFUSE_BASE_URL", "LANGFUSE_HOST"] {
        if let Ok(v) = std::env::var(key) {
            let trimmed = v.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed.trim_end_matches('/').to_string();
            }
        }
    }
    "https://us.cloud.langfuse.com".into()
}

fn observability_backend() -> String {
    let v = std::env::var("OBSERVABILITY_BACKEND")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if v.is_empty() {
        if langfuse_enabled() {
            "langfuse".into()
        } else {
            "none".into()
        }
    } else {
        v
    }
}

fn langfuse_auth_header() -> String {
    let pub_key = std::env::var("LANGFUSE_PUBLIC_KEY").unwrap_or_default();
    let sec_key = std::env::var("LANGFUSE_SECRET_KEY").unwrap_or_default();
    let tok = BASE64.encode(format!("{}:{}", pub_key.trim(), sec_key.trim()));
    format!("Basic {}", tok)
}

async fn langfuse_do(
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<(u16, Vec<u8>), reqwest::Error> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap();

    let url = format!("{}{}", langfuse_base(), path);
    let http_method: axum::http::Method = method.parse().unwrap_or(axum::http::Method::GET);

    let mut req = client.request(http_method.clone(), &url);
    req = req.header("Authorization", langfuse_auth_header());
    req = req.header("Content-Type", "application/json");

    if let Some(b) = body {
        req = req.json(&b);
    }

    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let raw = resp.bytes().await?.to_vec();
    Ok((status, raw))
}

pub async fn langfuse_status() -> impl IntoResponse {
    let mut out: serde_json::Map<String, Value> = serde_json::Map::new();
    out.insert("enabled".into(), json!(langfuse_enabled()));
    out.insert("backend".into(), json!(observability_backend()));
    out.insert("base_url".into(), json!(langfuse_base()));

    if !langfuse_enabled() {
        return (axum::http::StatusCode::OK, Json(Value::Object(out))).into_response();
    }

    if let Ok((code, raw)) = langfuse_do("GET", "/api/public/health", None).await {
        out.insert("health_status".into(), json!(code));
        if let Ok(health) = serde_json::from_slice::<Value>(&raw) {
            out.insert("health".into(), health);
        }
    }

    if let Ok((code, raw)) = langfuse_do("GET", "/api/public/projects", None).await {
        if code < 300 {
            if let Ok(projects) = serde_json::from_slice::<Value>(&raw) {
                out.insert("projects".into(), projects);
            }
        }
    }
    out.insert("dashboard_url".into(), json!(langfuse_base()));

    if let Ok((code, raw)) = langfuse_do("GET", "/api/public/llm-connections?limit=20", None).await
    {
        if code < 300 {
            if let Ok(conns) = serde_json::from_slice::<Value>(&raw) {
                out.insert("llm_connections".into(), conns);
            }
        }
    }

    if let Ok((code, raw)) =
        langfuse_do("GET", "/api/public/unstable/evaluators?limit=50", None).await
    {
        if code < 300 {
            if let Ok(evals) = serde_json::from_slice::<Value>(&raw) {
                out.insert("evaluators".into(), evals);
            }
        }
    }

    if let Ok((code, raw)) = langfuse_do(
        "GET",
        "/api/public/unstable/evaluation-rules?limit=50",
        None,
    )
    .await
    {
        if code < 300 {
            if let Ok(rules) = serde_json::from_slice::<Value>(&raw) {
                out.insert("evaluation_rules".into(), rules);
            }
        }
    }

    (axum::http::StatusCode::OK, Json(Value::Object(out))).into_response()
}

pub async fn langfuse_setup(
    Json(req): Json<SetupRequest>,
) -> Result<Json<Value>, crate::error::AppError> {
    if !langfuse_enabled() {
        return Err(crate::error::AppError::ServiceUnavailable(
            "langfuse_disabled".into(),
        ));
    }

    let max_cells = if req.max_cells > 0 {
        req.max_cells
    } else {
        40
    };

    let cells = load_current_cells().unwrap_or_default();
    let cells: Vec<&crate::domain::Cell> = cells.iter().take(max_cells).collect();

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    let mut batch: Vec<Value> = Vec::new();
    let mut trace_ids: Vec<String> = Vec::new();

    for c in &cells {
        let tid = Uuid::new_v4().to_string();
        let oid = Uuid::new_v4().to_string();
        trace_ids.push(tid.clone());

        let gen_ok = if c.gen_ok == 0.0 && c.pass_at_1 != 0.0 {
            c.pass_at_1
        } else {
            c.gen_ok
        };

        let inp = json!({
            "prompt": truncate(&c.prompt, 2000),
            "suite": c.suite,
            "task_id": c.task_id,
            "variant": c.variant,
        });
        let out_body = json!({
            "reply": truncate(&c.reply, 2000),
            "ok": c.ok,
            "gen_ok": gen_ok,
            "partial_credit": c.partial_credit,
            "pass_at_1": c.pass_at_1,
            "wall_clock_s": c.wall_clock_s,
            "tokens_per_second": c.tokens_per_second,
        });

        batch.push(json!({
            "id": Uuid::new_v4().to_string(),
            "type": "trace-create",
            "timestamp": now,
            "body": {
                "id": tid,
                "name": format!("{}/{}/{}", c.suite, c.task_id, c.variant),
                "tags": ["bench-cockpit", &c.suite, &c.variant],
                "metadata": {
                    "suite": c.suite,
                    "task_id": c.task_id,
                    "variant": c.variant,
                    "gen_ok": gen_ok,
                    "verified_pass_at_1": c.verified_pass_at_1,
                    "pass_at_1": c.pass_at_1,
                    "partial_credit": c.partial_credit,
                    "wall_clock_s": c.wall_clock_s,
                    "tokens_per_second": c.tokens_per_second,
                    "scoring_method": c.scoring_method,
                    "source": "bench-cockpit",
                    "rlvr_composite": c.rlvr_composite,
                    "rlvr_l0": c.rlvr_l0,
                    "rlvr_l1": c.rlvr_l1,
                    "rlvr_l2": c.rlvr_l2,
                    "rlvr_l3": c.rlvr_l3,
                    "rlvr_reward": c.rlvr_reward,
                    "rlvr_passed": c.rlvr_passed,
                    "rlvr_verifiable": c.rlvr_verifiable,
                    "rlvr_tournament_delta": c.rlvr_tournament_delta,
                },
                "input": inp,
                "output": out_body,
            }
        }));

        batch.push(json!({
            "id": Uuid::new_v4().to_string(),
            "type": "generation-create",
            "timestamp": now,
            "body": {
                "id": oid,
                "traceId": tid,
                "name": "bench-cell",
                "model": c.variant,
                "input": inp,
                "output": out_body,
                "startTime": now,
                "endTime": now,
                "metadata": {
                    "suite": c.suite,
                    "task_id": c.task_id,
                    "variant": c.variant,
                    "source": "bench-cockpit",
                },
            }
        }));

        batch.push(json!({
            "id": Uuid::new_v4().to_string(),
            "type": "score-create",
            "timestamp": now,
            "body": {
                "id": Uuid::new_v4().to_string(),
                "traceId": tid,
                "observationId": oid,
                "name": "gen_ok",
                "value": gen_ok,
                "dataType": "NUMERIC",
                "comment": "generation success (not verified pass@1)",
            }
        }));

        if c.partial_credit > 0.0 {
            batch.push(json!({
                "id": Uuid::new_v4().to_string(),
                "type": "score-create",
                "timestamp": now,
                "body": {
                    "id": Uuid::new_v4().to_string(),
                    "traceId": tid,
                    "observationId": oid,
                    "name": "partial_credit",
                    "value": c.partial_credit,
                    "dataType": "NUMERIC",
                }
            }));
        }

        if c.rlvr_composite > 0.0 || c.rlvr_reward > 0.0 || c.rlvr_verifiable {
            batch.push(json!({
                "id": Uuid::new_v4().to_string(),
                "type": "score-create",
                "timestamp": now,
                "body": {
                    "id": Uuid::new_v4().to_string(),
                    "traceId": tid,
                    "observationId": oid,
                    "name": "rlvr_composite",
                    "value": c.rlvr_composite,
                    "dataType": "NUMERIC",
                    "comment": "harness RLVR-AF composite",
                }
            }));
            for (name, val) in [
                ("rlvr_l0", c.rlvr_l0),
                ("rlvr_l1", c.rlvr_l1),
                ("rlvr_l2", c.rlvr_l2),
                ("rlvr_l3", c.rlvr_l3),
            ] {
                if val != 0.0 {
                    batch.push(json!({
                        "id": Uuid::new_v4().to_string(),
                        "type": "score-create",
                        "timestamp": now,
                        "body": {
                            "id": Uuid::new_v4().to_string(),
                            "traceId": tid,
                            "observationId": oid,
                            "name": name,
                            "value": val,
                            "dataType": "NUMERIC",
                        }
                    }));
                }
            }
        }
    }

    let ingestion_body = json!({"batch": batch});
    let (code, raw) = langfuse_do("POST", "/api/public/ingestion", Some(ingestion_body))
        .await
        .map_err(|e| crate::error::AppError::BadGateway(e.to_string()))?;

    let parsed: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);

    Ok(Json(json!({
        "status_code": code,
        "cells_seeded": cells.len(),
        "events": batch.len(),
        "trace_ids": trace_ids,
        "ingestion": parsed,
        "dashboard_url": langfuse_base(),
        "backend": "langfuse",
    })))
}

pub async fn langfuse_traces(
    Query(q): Query<TracesQuery>,
) -> Result<(axum::http::StatusCode, HeaderMap, Vec<u8>), crate::error::AppError> {
    if !langfuse_enabled() {
        return Err(crate::error::AppError::ServiceUnavailable(
            "langfuse_disabled".into(),
        ));
    }

    let (code, raw) = langfuse_do("GET", &format!("/api/public/traces?limit={}", q.limit), None)
        .await
        .map_err(|e| crate::error::AppError::BadGateway(e.to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    let status = axum::http::StatusCode::from_u16(code).unwrap_or(axum::http::StatusCode::OK);
    Ok((status, headers, raw))
}

pub async fn langfuse_evaluators(
    Query(q): Query<EvaluatorsQuery>,
) -> Result<Json<Value>, crate::error::AppError> {
    if !langfuse_enabled() {
        return Err(crate::error::AppError::ServiceUnavailable(
            "langfuse_disabled".into(),
        ));
    }

    let evals_root = evals_root();
    let script = format!("{}/scripts/evals/run_langfuse_evaluators.py", evals_root);
    let python = evals_python();

    let output = tokio::process::Command::new(&python)
        .arg(&script)
        .arg(&q.action)
        .arg("--limit")
        .arg(&q.limit)
        .current_dir(&evals_root)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let raw_out = stdout.trim();
            let mut result: serde_json::Map<String, Value> = serde_json::Map::new();
            result.insert("stdout".into(), json!(stdout));
            result.insert("action".into(), json!(q.action));

            for line in raw_out.lines().rev() {
                let line = line.trim();
                if line.starts_with('{') {
                    if let Ok(parsed) = serde_json::from_str::<Value>(line) {
                        result.insert("result".into(), parsed);
                        break;
                    }
                }
            }

            if !output.status.success() {
                let err_msg = if stderr.is_empty() {
                    "command failed".into()
                } else {
                    stderr
                };
                result.insert("error".into(), json!(err_msg));
            }

            Ok(Json(Value::Object(result)))
        }
        Err(e) => Err(crate::error::AppError::BadGateway(e.to_string())),
    }
}
