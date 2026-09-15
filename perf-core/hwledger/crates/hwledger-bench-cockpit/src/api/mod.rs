pub mod langfuse;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::analysis::{build_suite_coverage, lint_cells};
use crate::domain::{summarize_by_variant, Envelope, ResultsData};
use crate::helpers::truncate;
use crate::error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub data_path: String,
    pub extra_paths: Vec<String>,
    #[allow(dead_code)]
    pub dist_dir: String,
    pub ring: Arc<RwLock<Vec<Envelope>>>,
}

impl AppState {
    pub fn new(data_path: String, extra_paths: Vec<String>, dist_dir: String) -> Self {
        Self {
            data_path,
            extra_paths,
            dist_dir,
            ring: Arc::new(RwLock::new(Vec::with_capacity(30))),
        }
    }
}

fn load_results_file(path: &str) -> Result<ResultsData, AppError> {
    let raw = std::fs::read(path).map_err(|e| AppError::Internal(format!("read {}: {}", path, e)))?;
    let data: ResultsData =
        serde_json::from_slice(&raw).map_err(|e| AppError::Internal(format!("unmarshal {}: {}", path, e)))?;
    if data.cells.is_empty() {
        return Err(AppError::Internal(format!("{} has 0 cells", path)));
    }
    Ok(data)
}

fn load_data(state: &AppState) -> Result<ResultsData, AppError> {
    let mut base = load_results_file(&state.data_path)?;
    for extra in &state.extra_paths {
        match load_results_file(extra) {
            Ok(ex) => merge_results(&mut base, ex),
            Err(e) => tracing::warn!("skip extra data {}: {}", extra, e),
        }
    }
    Ok(base)
}

fn merge_results(base: &mut ResultsData, extra: ResultsData) {
    let seen: HashMap<String, ()> = base
        .cells
        .iter()
        .map(|c| format!("{}\0{}\0{}", c.suite, c.task_id, c.variant))
        .map(|k| (k, ()))
        .collect();

    for c in extra.cells {
        let key = format!("{}\0{}\0{}", c.suite, c.task_id, c.variant);
        if !seen.contains_key(&key) {
            base.cells.push(c);
        }
    }

    base.summary.by_variant = summarize_by_variant(&base.cells);
    let suites: HashMap<&str, ()> = base.cells.iter().map(|c| (c.suite.as_str(), ())).collect();
    let variants: HashMap<&str, ()> = base.cells.iter().map(|c| (c.variant.as_str(), ())).collect();
    base.summary.meta.n_cells = base.cells.len();
    base.summary.meta.n_suites = suites.len();
    base.summary.meta.variants = variants.keys().map(|s| s.to_string()).collect();

    let ablation: Vec<&str> = ["stock", "ours"]
        .iter()
        .filter(|v| variants.contains_key(*v))
        .copied()
        .collect();
    if !ablation.is_empty() {
        base.summary.meta.model = ablation.join("+");
    } else if base.summary.meta.model.is_empty() && !base.summary.meta.variants.is_empty() {
        base.summary.meta.model = base.summary.meta.variants.join("+");
    }
}

pub async fn build_envelope(state: &AppState) -> Result<Envelope, AppError> {
    let data = load_data(state)?;
    let warnings = lint_cells(&data.cells);
    let suite_coverage = build_suite_coverage(&data.cells);
    Ok(Envelope {
        json_path: state.data_path.clone(),
        extra_paths: Some(state.extra_paths.clone()),
        server_ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        data: Some(data),
        warnings: Some(warnings),
        lint_run_ts: Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)),
        suite_coverage: Some(suite_coverage),
    })
}

pub async fn health() -> impl IntoResponse {
    let data_path = std::env::var("BENCH_DATA_PATH")
        .or_else(|_| std::env::var("DATA_PATH"))
        .unwrap_or_default();
    Json(json!({
        "status": "ok",
        "jsonPath": data_path,
        "ts": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
    }))
}

pub async fn api_state(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let env = build_envelope(&state).await?;
    Ok(Json(serde_json::to_value(env).unwrap_or_default()))
}

pub async fn api_history(State(state): State<Arc<AppState>>) -> Json<Value> {
    let ring = state.ring.read().await;
    Json(serde_json::to_value(&*ring).unwrap_or_default())
}

pub async fn api_export(State(state): State<Arc<AppState>>) -> Result<Response, AppError> {
    let env = build_envelope(&state).await?;
    let body = serde_json::to_vec_pretty(&env).unwrap_or_default();
    let filename = std::path::Path::new(&state.data_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "results.json".into());

    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    headers.insert(
        "content-disposition",
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );
    Ok((headers, body).into_response())
}

pub async fn api_cell_raw(
    Path((suite, task_id, variant)): Path<(String, String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, AppError> {
    let cells = {
        let ring = state.ring.read().await;
        ring.last()
            .and_then(|env| env.data.as_ref().map(|d| d.cells.clone()))
            .unwrap_or_default()
    };

    let cells = if cells.is_empty() {
        load_data(&state).map(|d| d.cells)?
    } else {
        cells
    };

    for c in &cells {
        if c.suite == suite && c.task_id == task_id && c.variant == variant {
            return Ok(Json(json!({
                "suite": c.suite,
                "task_id": c.task_id,
                "variant": c.variant,
                "task_title": c.task_title,
                "task_description": c.task_description,
                "acceptance": c.acceptance,
                "rubric": c.rubric,
                "assignment": c.assignment,
                "prompt": c.prompt,
                "reply": c.reply,
                "reply_full": c.reply_full.as_deref().unwrap_or(&c.reply),
                "expected_answer": c.expected_answer,
                "scoring_method": c.scoring_method,
                "pass_at_1": c.pass_at_1,
                "gen_ok": if c.gen_ok != 0.0 { c.gen_ok } else { c.pass_at_1 },
                "verified_pass_at_1": c.verified_pass_at_1,
                "judge_score": c.judge_score,
                "failure_analysis": c.failure_analysis,
                "progress_trace": c.progress_trace,
                "chat_trace": c.chat_trace,
                "wall_clock_s": c.wall_clock_s,
                "tokens_per_second": c.tokens_per_second,
            })));
        }
    }

    Err(AppError::NotFound("cell_not_found".into()))
}

pub async fn api_eval_run(
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let root = std::env::var("PORTAGE_ROOT")
        .map_err(|_| AppError::BadRequest("portage_root_required".into()))?;

    let root_path = std::path::Path::new(&root);
    if !root_path.is_dir() {
        return Err(AppError::BadRequest(format!(
            "PORTAGE_ROOT is not a directory: {}",
            root
        )));
    }

    let run_id = format!("run-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let jobs_dir = std::env::var("PORTAGE_JOBS_DIR")
        .unwrap_or_else(|_| format!("{}/bench-cockpit-portage-jobs", std::env::temp_dir().display()));
    let dir = format!("{}/{}", jobs_dir, run_id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let portage_bin = std::env::var("PORTAGE_BIN").unwrap_or_else(|_| "uv run harbor".into());
    let parts: Vec<&str> = portage_bin.split_whitespace().collect();
    if parts.is_empty() {
        return Err(AppError::Internal("PORTAGE_BIN empty".into()));
    }

    let mode = body.get("mode").and_then(|v| v.as_str()).unwrap_or("");
    let has_tasks = body.get("tasks").is_some();
    let use_config = has_tasks && mode != "path" && mode != "hello_world";

    let mut args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    if use_config {
        let job_path = format!("{}/job.json", dir);
        let raw = serde_json::to_string_pretty(&body).unwrap_or_default();
        std::fs::write(&job_path, raw)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        args.extend(["run".into(), "-c".into(), job_path, "-o".into(), dir.clone(), "-y".into()]);
    } else {
        let mut task_path = body
            .get("task_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if task_path.is_empty() || mode == "hello_world" || mode.is_empty() {
            task_path = format!("{}/examples/tasks/hello-world", root);
        }
        let agent = body
            .get("agent")
            .and_then(|v| v.as_str())
            .unwrap_or("oracle");
        let n = body
            .get("n")
            .and_then(|v| v.as_f64())
            .filter(|&v| v >= 1.0)
            .map(|v| (v as usize).to_string())
            .unwrap_or_else(|| "1".into());

        args.extend([
            "run".into(),
            "-e".into(),
            "apple-container".into(),
            "-p".into(),
            task_path,
            "-a".into(),
            agent.into(),
            "-n".into(),
            n,
            "-o".into(),
            dir.clone(),
            "-y".into(),
            "--plugin".into(),
            "langfuse".into(),
        ]);
    }

    let output = tokio::process::Command::new(parts[0])
        .args(&args)
        .current_dir(&root)
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::ServiceUnavailable(format!("binary not found: {}", parts[0]))
            } else {
                AppError::Internal(e.to_string())
            }
        })?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
    let _ = std::fs::write(format!("{}/stdout.log", dir), &stdout_str);
    let _ = std::fs::write(
        format!("{}/cmdline.txt", dir),
        format!("{} {}", parts[0], args.join(" ")),
    );

    if !output.status.success() {
        return Err(AppError::BadRequest(format!(
            "portage_run_rejected: {}",
            truncate(&stdout_str, 4000)
        )));
    }

    let (result_path, reward) = find_harbor_result(&dir);

    Ok(Json(json!({
        "run_id": run_id,
        "job_dir": dir,
        "status": "completed",
        "result_path": result_path,
        "reward": reward,
    })))
}

fn find_harbor_result(run_dir: &str) -> (String, Option<Value>) {
    let mut candidates = vec![format!("{}/result.json", run_dir)];
    if let Ok(entries) = std::fs::read_dir(run_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                candidates.push(format!("{}/result.json", entry.path().display()));
            }
        }
    }
    for path in &candidates {
        if let Ok(raw) = std::fs::read(path) {
            if let Ok(parsed) = serde_json::from_slice::<Value>(&raw) {
                return (path.clone(), extract_reward(&parsed));
            }
        }
    }
    (String::new(), None)
}

fn extract_reward(parsed: &Value) -> Option<Value> {
    parsed
        .get("stats")
        .and_then(|s| s.get("evals"))
        .cloned()
        .or_else(|| parsed.get("stats").cloned())
        .or_else(|| parsed.get("reward").cloned())
}

pub async fn api_eval_run_status(
    Path(id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, AppError> {
    if id.contains('/') || id.contains("..") {
        return Err(AppError::BadRequest("run_id required".into()));
    }

    let jobs_dir = std::env::var("PORTAGE_JOBS_DIR")
        .unwrap_or_else(|_| format!("{}/bench-cockpit-portage-jobs", std::env::temp_dir().display()));
    let run_dir = format!("{}/{}", jobs_dir, id);

    let (result_path, reward) = find_harbor_result(&run_dir);
    if result_path.is_empty() {
        let stdout = std::fs::read_to_string(format!("{}/stdout.log", run_dir))
            .unwrap_or_default();
        return Err(AppError::NotFound(format!(
            "result_not_found: {}",
            truncate(&stdout, 2000)
        )));
    }

    let raw = std::fs::read(&result_path).unwrap_or_default();
    let parsed: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);

    Ok(Json(json!({
        "run_id": id,
        "result_path": result_path,
        "reward": reward,
        "result": parsed,
    })))
}

pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(
    mut socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
) {
    // Send initial reload notify
    if let Ok(env) = build_envelope(&state).await {
        if let Ok(notify) = serde_json::to_string(&json!({
            "type": "reload",
            "serverTs": env.server_ts,
        })) {
            let _ = socket
                .send(axum::extract::ws::Message::Text(notify.into()))
                .await;
        }
    }

    // Keep connection alive — just read pings/pongs
    while let Some(Ok(_)) = socket.recv().await {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello");
    }
}
