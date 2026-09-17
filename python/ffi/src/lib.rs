// phenotype-omlx FFI — pyo3 bridge exposing the Rust perf-core to Python.
//
// Build:
//   cd /Users/<REDACTED>/CodeProjects/Phenotype/repos/phenotype-omlx/python/ffi
//   maturin develop --release --features extension-module

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;
use tokio::runtime::Runtime;

use concurrent_exec::{
    jetspec::JetSpecBackend, latentmas::LatentMasBackend, plan::AgentId, ssd::SsdBackend,
    tidar::TidarAgent, ExecBackend, ExecRequest as RustExecRequest, ExecResult as RustExecResult,
};
use spec_decode::build_engine;
use spec_decode::SpecDecodeEngine;

// ── Module imports ─────────────────────────────────────────────────────────
#[allow(dead_code)]
mod agent_ffi;
mod mlx_backend_ffi;
mod spec_decode_ffi;
mod tree_attention_ffi;
mod turbo_quant_ffi;

use mlx_backend_ffi::{
    MlxTargetBackend, MlxTargetBackendClone, NullTargetBackend, PyMlxTargetBackend,
};
use spec_decode_ffi::{PyDraftMode, PySpecDecodeConfig};
use tree_attention_ffi::{tree_attn_causal_mask, PyTreePlan};
use turbo_quant_ffi::{turbo_quant_decode, turbo_quant_encode, turbo_quant_label_for_bits};

// ── Spec-decode engine wrapper ─────────────────────────────────────────────

#[pyclass]
struct PySpecDecodeEngine {
    inner: Arc<tokio::sync::Mutex<SpecDecodeEngine>>,
}

#[pymethods]
impl PySpecDecodeEngine {
    #[new]
    #[pyo3(signature = (cfg, target=None, draft=None))]
    fn new(
        cfg: &PySpecDecodeConfig,
        target: Option<&Bound<'_, PyAny>>,
        draft: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        // Resolve target backend: a PyMlxTargetBackend instance wins;
        // otherwise fall back to NullTargetBackend. We can't downcast to
        // a foreign type across pyo3 boundaries, so we read the
        // `inner` attribute convention we set on PyMlxTargetBackend.
        let target_box: Box<dyn spec_decode::backend::TargetBackend> = if let Some(t) = target {
            // Two ways to pass: a PyMlxTargetBackend instance (we already
            // built the backend); or a string model id (we build a real
            // MLX target for them). This makes the API forgiving.
            if let Ok(py_mlx) = t.extract::<PyMlxTargetBackend>() {
                Box::new(MlxTargetBackendClone::from(py_mlx))
            } else if let Ok(model_id) = t.extract::<String>() {
                let be = MlxTargetBackend::build(&model_id, None)?;
                Box::new(be)
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "target must be a _perf.MlxTargetBackend or a model id string",
                ));
            }
        } else {
            Box::new(NullTargetBackend)
        };

        // Draft backend: only NullDraftBackend for now (SameModel mode is
        // handled inside the engine via prompt-lookup). Future: a real
        // PyMlxDraftBackend mirroring the target pattern.
        let _ = &draft;
        let draft_box: Option<Box<dyn spec_decode::backend::DraftBackend>> =
            Some(Box::new(spec_decode::backend::NullDraftBackend));

        let engine = build_engine(cfg.inner.clone(), target_box, draft_box);
        Ok(Self { inner: engine })
    }

    fn config_summary(&self, py: Python<'_>) -> PyResult<String> {
        // Hold the GIL while calling .blocking_lock() — that's the tokio Mutex
        // API that lets us avoid deadlocking against the runtime.
        let g = self.inner.blocking_lock();
        let _ = py;
        Ok(format!(
            "SpecDecodeEngine {{ mode={:?}, max_draft_tokens={} }}",
            g.config.mode, g.config.max_draft_tokens
        ))
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn py_to_exec_request(req: &Bound<'_, PyDict>) -> PyResult<RustExecRequest> {
    let stop: Vec<String> = match req.get_item("stop").ok().flatten() {
        Some(v) => v.extract()?,
        None => Vec::new(),
    };
    Ok(RustExecRequest {
        prompt: req
            .get_item("prompt")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("prompt"))?
            .extract()?,
        max_tokens: req
            .get_item("max_tokens")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("max_tokens"))?
            .extract()?,
        temperature: req
            .get_item("temperature")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("temperature"))?
            .extract()?,
        stop,
    })
}

fn exec_result_to_py(py: Python, r: RustExecResult) -> PyResult<Py<PyAny>> {
    let d = PyDict::new(py);
    d.set_item("text", r.text)?;
    d.set_item("tokens", r.tokens)?;
    d.set_item("elapsed_ms", r.elapsed_ms)?;
    Ok(d.into())
}

fn runtime() -> PyResult<Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

fn device_arc(d: Option<String>) -> Arc<str> {
    Arc::from(d.unwrap_or_else(|| "cpu".into()).into_boxed_str())
}

// ── Concurrent-exec agent runners ──────────────────────────────────────────

// LatentMAS
#[pyfunction]
#[pyo3(signature = (n_agents, req, device=None))]
fn run_latentmas(
    py: Python<'_>,
    n_agents: usize,
    req: &Bound<'_, PyDict>,
    device: Option<String>,
) -> PyResult<Py<PyAny>> {
    let backend = LatentMasBackend::new(n_agents, device_arc(device));
    let exec_req = py_to_exec_request(req)?;
    let rt = runtime()?;
    let res = rt
        .block_on(backend.run(AgentId::new("latentmas"), exec_req))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    exec_result_to_py(py, res)
}

// TiDAR
#[pyfunction]
#[pyo3(signature = (draft_len, diff_steps, req, device=None))]
fn run_tidar(
    py: Python<'_>,
    draft_len: usize,
    diff_steps: usize,
    req: &Bound<'_, PyDict>,
    device: Option<String>,
) -> PyResult<Py<PyAny>> {
    let backend = TidarAgent::drafter(draft_len, diff_steps, device_arc(device));
    let exec_req = py_to_exec_request(req)?;
    let rt = runtime()?;
    let res = rt
        .block_on(backend.run(AgentId::new("tidar"), exec_req))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    exec_result_to_py(py, res)
}

// JetSpec
#[pyfunction]
#[pyo3(signature = (tree_width, tree_depth, req, device=None))]
fn run_jetspec(
    py: Python<'_>,
    tree_width: usize,
    tree_depth: usize,
    req: &Bound<'_, PyDict>,
    device: Option<String>,
) -> PyResult<Py<PyAny>> {
    let backend = JetSpecBackend::new(tree_width, tree_depth, device_arc(device));
    let exec_req = py_to_exec_request(req)?;
    let rt = runtime()?;
    let res = rt
        .block_on(backend.run(AgentId::new("jetspec"), exec_req))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    exec_result_to_py(py, res)
}

// SSD
#[pyfunction]
#[pyo3(signature = (gamma, req, device=None))]
fn run_ssd(
    py: Python<'_>,
    gamma: usize,
    req: &Bound<'_, PyDict>,
    device: Option<String>,
) -> PyResult<Py<PyAny>> {
    let backend = SsdBackend::new(gamma, device_arc(device));
    let exec_req = py_to_exec_request(req)?;
    let rt = runtime()?;
    let res = rt
        .block_on(backend.run(AgentId::new("ssd"), exec_req))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    exec_result_to_py(py, res)
}

// ── Module registration ────────────────────────────────────────────────────

#[pymodule]
fn _perf(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // turbo-quant
    m.add_function(wrap_pyfunction!(turbo_quant_label_for_bits, m)?)?;
    m.add_function(wrap_pyfunction!(turbo_quant_encode, m)?)?;
    m.add_function(wrap_pyfunction!(turbo_quant_decode, m)?)?;
    // tree-attention
    m.add_function(wrap_pyfunction!(tree_attn_causal_mask, m)?)?;
    // concurrent-exec agents
    m.add_function(wrap_pyfunction!(run_latentmas, m)?)?;
    m.add_function(wrap_pyfunction!(run_tidar, m)?)?;
    m.add_function(wrap_pyfunction!(run_jetspec, m)?)?;
    m.add_function(wrap_pyfunction!(run_ssd, m)?)?;
    // classes
    m.add_class::<PyDraftMode>()?;
    m.add_class::<PySpecDecodeConfig>()?;
    m.add_class::<PySpecDecodeEngine>()?;
    m.add_class::<PyTreePlan>()?;
    m.add_class::<PyMlxTargetBackend>()?;
    Ok(())
}
