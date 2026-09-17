// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::watch;
use uuid::Uuid;

use agileplus_graph::RelType;

use crate::{Graph, PipelineError, ResourceLimits};

/// Result of executing a graph.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub node_outputs: HashMap<Uuid, NodeOutput>,
    pub final_status: ExecutionStatus,
    pub started_at: Instant,
    pub finished_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Success,
    Partial,
    Failed,
}

/// Per-node execution output.
#[derive(Debug, Clone)]
pub struct NodeOutput {
    pub success: bool,
    pub skipped: bool,
    pub stdout_path: Option<PathBuf>,
    pub stderr_path: Option<PathBuf>,
    pub exit_code: Option<i32>,
    pub attempts: u32,
    pub started_at: Instant,
    pub finished_at: Instant,
    pub last_error: Option<String>,
}

/// Executor topologically sorts the graph and runs nodes in parallel where possible.
#[derive(Debug, Clone)]
pub struct Executor {
    /// Default timeout for nodes without an explicit timeout attribute.
    /// Defaults to 60 seconds via [`Executor::new`] / [`Default`].
    pub default_timeout_secs: u64,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Self {
            default_timeout_secs: 60,
        }
    }

    /// Execute the graph.
    ///
    /// 1. Build a petgraph DAG for topological sort.
    /// 2. For each node, wait for dependencies to finish.
    /// 3. Evaluate guard edges before deciding to run.
    /// 4. Spawn tokio tasks for shell commands.
    /// 5. Capture stdout/stderr to temp files.
    /// 6. Retry on failure with exponential backoff.
    pub async fn execute(
        &self,
        graph: &Graph,
        _limits: &ResourceLimits,
    ) -> anyhow::Result<ExecutionResult> {
        let started_at = Instant::now();
        let mut node_outputs: HashMap<Uuid, NodeOutput> = HashMap::new();
        let mut completed: HashSet<Uuid> = HashSet::new();
        let mut skipped: HashSet<Uuid> = HashSet::new();

        // Build petgraph for topo sort.
        let mut pet_graph = petgraph::graph::DiGraph::<Uuid, ()>::new();
        let mut idx_to_uuid = HashMap::new();
        let mut uuid_to_idx = HashMap::new();

        for &id in graph.nodes.keys() {
            let idx = pet_graph.add_node(id);
            idx_to_uuid.insert(idx, id);
            uuid_to_idx.insert(id, idx);
        }

        // Add edges from dependency -> dependent (so topo sort gives us dependency first).
        for rel in &graph.relationships {
            if rel.rel_type == RelType::DependsOn || rel.rel_type == RelType::Blocks {
                // dependent (from) depends on dependency (to)
                // Edge direction in execution graph: to -> from
                if let (Some(&from), Some(&to)) = (
                    uuid_to_idx.get(&rel.to_node_id),
                    uuid_to_idx.get(&rel.from_node_id),
                ) {
                    pet_graph.add_edge(from, to, ());
                }
            }
        }

        let topo = petgraph::algo::toposort(&pet_graph, None)
            .map_err(|cycle| PipelineError::Execution(format!("Cycle detected: {cycle:?}")))?;

        // Channel to notify dependents when a node finishes.
        let mut senders: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
        let mut receivers: HashMap<Uuid, watch::Receiver<bool>> = HashMap::new();

        for &id in graph.nodes.keys() {
            let (tx, rx) = watch::channel(false);
            senders.insert(id, tx);
            receivers.insert(id, rx);
        }

        let mut handles = Vec::new();

        for node_idx in topo {
            let node_id = idx_to_uuid[&node_idx];
            let node = graph.get_node(node_id).cloned().unwrap();

            // Collect dependency and blocker IDs.
            let deps: Vec<Uuid> = graph.dependencies(node_id);
            let blockers: Vec<Uuid> = graph.blockers(node_id);
            let incoming: Vec<_> = graph.incoming(node_id);

            let mut guard_rx = Vec::new();
            for dep_id in &deps {
                guard_rx.push(receivers.get(dep_id).cloned().unwrap());
            }
            for blocker_id in &blockers {
                guard_rx.push(receivers.get(blocker_id).cloned().unwrap());
            }

            let guard_edges: Vec<_> = incoming
                .iter()
                .filter(|r| r.properties.get("guard").is_some())
                .cloned()
                .cloned()
                .collect();

            let node_output_tx = senders.get(&node_id).cloned().unwrap();
            let default_timeout = self.default_timeout_secs;

            let handle = tokio::spawn(async move {
                let started_at = Instant::now();

                // Wait for all dependencies / blockers to finish.
                for mut rx in guard_rx {
                    let _ = rx.changed().await;
                }

                // Evaluate guard edges: if any guard fails, skip this node.
                for edge in &guard_edges {
                    if let Some(guard_cmd) = edge.properties.get("guard").and_then(|v| v.as_str()) {
                        let status = Command::new("sh").arg("-c").arg(guard_cmd).status().await;
                        match status {
                            Ok(s) if s.code() == Some(0) => {}
                            _ => {
                                let _ = node_output_tx.send(true);
                                return (
                                    node_id,
                                    NodeOutput {
                                        success: false,
                                        skipped: true,
                                        stdout_path: None,
                                        stderr_path: None,
                                        exit_code: None,
                                        attempts: 0,
                                        started_at,
                                        finished_at: Instant::now(),
                                        last_error: Some(format!("Guard failed: {guard_cmd}")),
                                    },
                                );
                            }
                        }
                    }
                }

                // Extract node attributes.
                let cmd_str = node
                    .properties
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let working_dir = node
                    .properties
                    .get("working_dir")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from);
                let retries = node
                    .properties
                    .get("retries")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let timeout_secs = node
                    .properties
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(default_timeout);

                let mut attempts = 0u32;
                #[allow(unused_assignments)]
                let mut last_result: Option<NodeOutput> = None;

                if cmd_str.is_empty() {
                    // No command — treat as no-op success.
                    let _ = node_output_tx.send(true);
                    return (
                        node_id,
                        NodeOutput {
                            success: true,
                            skipped: false,
                            stdout_path: None,
                            stderr_path: None,
                            exit_code: Some(0),
                            attempts: 0,
                            started_at,
                            finished_at: Instant::now(),
                            last_error: None,
                        },
                    );
                }

                loop {
                    attempts += 1;
                    let stdout_file = tempfile::NamedTempFile::new().ok();
                    let stderr_file = tempfile::NamedTempFile::new().ok();

                    #[cfg(target_os = "windows")]
                    let mut cmd = {
                        let mut c = Command::new("cmd");
                        c.args(["/C", cmd_str.as_str()]);
                        c
                    };
                    #[cfg(not(target_os = "windows"))]
                    let mut cmd = {
                        let mut c = Command::new("sh");
                        c.arg("-c").arg(&cmd_str);
                        c
                    };
                    if let Some(ref dir) = working_dir {
                        cmd.current_dir(dir);
                    }
                    if let Some(ref f) = stdout_file {
                        let std_file = std::fs::File::create(f.path())
                            .ok()
                            .map(std::process::Stdio::from);
                        if let Some(f) = std_file {
                            cmd.stdout(f);
                        }
                    }
                    if let Some(ref f) = stderr_file {
                        let std_file = std::fs::File::create(f.path())
                            .ok()
                            .map(std::process::Stdio::from);
                        if let Some(f) = std_file {
                            cmd.stderr(f);
                        }
                    }

                    let result =
                        tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.status()).await;

                    let (success, exit_code, error) = match result {
                        Ok(Ok(status)) => {
                            let code = status.code();
                            let ok = code == Some(0);
                            (
                                ok,
                                code,
                                if ok {
                                    None
                                } else {
                                    Some(format!("Non-zero exit: {code:?}"))
                                },
                            )
                        }
                        Ok(Err(e)) => (false, None, Some(format!("Spawn error: {e}"))),
                        Err(_) => (false, None, Some(format!("Timeout after {timeout_secs}s"))),
                    };

                    let stdout_path = stdout_file.map(|f| f.into_temp_path().to_path_buf());
                    let stderr_path = stderr_file.map(|f| f.into_temp_path().to_path_buf());

                    last_result = Some(NodeOutput {
                        success,
                        skipped: false,
                        stdout_path,
                        stderr_path,
                        exit_code,
                        attempts,
                        started_at,
                        finished_at: Instant::now(),
                        last_error: error,
                    });

                    if success || attempts > retries {
                        break;
                    }

                    // Exponential backoff: 2^(attempt-1) seconds.
                    let backoff = Duration::from_secs(2u64.saturating_pow(attempts - 1));
                    tokio::time::sleep(backoff).await;
                }

                let output = last_result.unwrap_or_else(|| NodeOutput {
                    success: false,
                    skipped: false,
                    stdout_path: None,
                    stderr_path: None,
                    exit_code: None,
                    attempts,
                    started_at,
                    finished_at: Instant::now(),
                    last_error: Some("No attempts made".into()),
                });

                let _ = node_output_tx.send(true);
                (node_id, output)
            });

            handles.push(handle);
        }

        // Wait for all tasks to finish and collect results.
        for handle in handles {
            let (node_id, output) = handle
                .await
                .map_err(|e| PipelineError::Execution(format!("Task join error: {e}")))?;
            if output.skipped || !output.success {
                skipped.insert(node_id);
            } else {
                completed.insert(node_id);
            }
            node_outputs.insert(node_id, output);
        }

        // Post-pass: mark nodes whose blockers failed as skipped.
        for node_id in graph.nodes.keys() {
            let blockers = graph.blockers(*node_id);
            if blockers.iter().any(|b| {
                node_outputs
                    .get(b)
                    .map(|o| !o.success && !o.skipped)
                    .unwrap_or(false)
            }) {
                if let Some(o) = node_outputs.get_mut(node_id) {
                    o.skipped = true;
                    o.success = false;
                    o.last_error = Some("Blocker failed".into());
                }
                skipped.insert(*node_id);
                completed.remove(node_id);
            }
        }

        let final_status = if skipped.is_empty() && completed.len() == graph.nodes.len() {
            ExecutionStatus::Success
        } else if completed.is_empty() {
            ExecutionStatus::Failed
        } else {
            ExecutionStatus::Partial
        };

        let finished_at = Instant::now();

        Ok(ExecutionResult {
            node_outputs,
            final_status,
            started_at,
            finished_at,
        })
    }
}
