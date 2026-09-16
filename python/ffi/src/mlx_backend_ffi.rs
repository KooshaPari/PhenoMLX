//! MLX target backend pyo3 bindings.
//!
//! Wraps an MLX model (via `mlx_lm.load`) as a `TargetBackend` for the
//! spec-decode engine. The GIL is dropped during model forward passes
//! via `spawn_blocking` + `Python::attach`.

use async_trait::async_trait;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use spec_decode::backend::{BackendInfo, TargetBackend, TargetOutput};
use std::sync::Arc;

/// Real MLX target backend. Stores the model and EOS token ids as
/// Python objects (cloned into spawn_blocking closures via Arc, then
/// re-attached via `Python::attach`).
pub(crate) struct MlxTargetBackend {
    model: Arc<Py<PyAny>>,
    model_id: String,
    eos_token_ids: Arc<Vec<u32>>,
    kv_cache_kind: Option<String>,
}

impl MlxTargetBackend {
    /// Build a new MlxTargetBackend from a model id / local path.
    pub(crate) fn build(model_id: &str, kv_cache_kind: Option<String>) -> PyResult<Self> {
        Python::attach(|py| {
            let mlx_lm = py.import("mlx_lm")?;
            let load = mlx_lm.getattr("load")?;
            let pair = load.call1((model_id,))?;
            let model: Py<PyAny> = pair.get_item(0)?.into();
            let tokenizer: Py<PyAny> = pair.get_item(1)?.into();

            // Pull EOS token ids from the tokenizer (some tokenizers expose
            // `.eos_token_id`; mlx_lm wraps with a TokenizerWrapper that has
            // `.eos_token_ids` as a set/list).
            let tok_bound = tokenizer.bind(py);
            let eos_ids: Vec<u32> = if let Ok(ids_any) = tok_bound.getattr("eos_token_ids") {
                if let Ok(v) = ids_any.extract::<Vec<u32>>() {
                    v
                } else if let Ok(s) = ids_any.extract::<std::collections::HashSet<u32>>() {
                    s.into_iter().collect()
                } else {
                    Vec::new()
                }
            } else if let Ok(id_any) = tok_bound.getattr("eos_token_id") {
                if let Ok(v) = id_any.extract::<u32>() {
                    vec![v]
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            Ok(MlxTargetBackend {
                model: Arc::new(model),
                model_id: model_id.to_string(),
                eos_token_ids: Arc::new(eos_ids),
                kv_cache_kind,
            })
        })
    }
}

#[async_trait]
impl TargetBackend for MlxTargetBackend {
    async fn forward(&self, token_ids: &[u32]) -> Result<TargetOutput, String> {
        // We need to drop the GIL for MLX work and yield back to the
        // tokio runtime so other tasks can run. `spawn_blocking` plus
        // `Python::attach` is the canonical pattern.
        let model = Arc::clone(&self.model);
        let eos = Arc::clone(&self.eos_token_ids);
        let ids: Vec<u32> = token_ids.to_vec();

        tokio::task::spawn_blocking(move || -> Result<TargetOutput, String> {
            Python::attach(|py| -> Result<TargetOutput, String> {
                let mx = py
                    .import("mlx.core")
                    .map_err(|e| format!("import mlx.core: {e}"))?;
                let ids_py = PyList::new(py, ids.iter().copied())
                    .map_err(|e| format!("build token id list: {e}"))?;
                // mx.array(ids) -> shape [seq]
                let prompt = mx
                    .call_method1("array", (ids_py,))
                    .map_err(|e| format!("mx.array: {e}"))?;
                // prompt[None] -> shape [1, seq] for batched forward
                let none = py.None();
                let batched = prompt
                    .call_method1("__getitem__", (none,))
                    .map_err(|e| format!("prompt[None]: {e}"))?;

                // Model output has shape [batch, sequence, vocabulary].
                let logits = model
                    .bind(py)
                    .call1((batched,))
                    .map_err(|e| format!("model forward: {e}"))?;
                let index = PyTuple::new(py, [0_i32, -1_i32])
                    .map_err(|e| format!("build logits index: {e}"))?;
                let last = logits
                    .call_method1("__getitem__", (index,))
                    .map_err(|e| format!("logits[0, -1]: {e}"))?;
                let logits_py = last
                    .call_method0("tolist")
                    .map_err(|e| format!("logits.tolist: {e}"))?;
                let logits_vec: Vec<f32> = logits_py
                    .extract()
                    .map_err(|e| format!("extract logits: {e}"))?;

                // Greedy next-token from the logits — this is what spec-decode
                // uses to score acceptance. For `SameModel` (prompt-lookup)
                // the engine doesn't actually use the logits, but for any
                // draft-tree verifier we do.
                let next_token = logits_vec
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i as u32)
                    .unwrap_or(0);

                let finished = eos.contains(&next_token);

                Ok(TargetOutput {
                    logits: logits_vec,
                    hidden: None,
                    finished,
                })
            })
        })
        .await
        .map_err(|e| format!("spawn_blocking join: {e}"))?
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            engine: "mlx".into(),
            model_id: self.model_id.clone(),
            device: "metal".into(),
            dtype: "float16".into(),
            kv_cache_type: self.kv_cache_kind.clone(),
        }
    }
}

/// Python wrapper around MlxTargetBackend.
#[pyclass(name = "MlxTargetBackend", from_py_object)]
#[derive(Clone)]
pub(crate) struct PyMlxTargetBackend {
    pub(crate) inner: Arc<MlxTargetBackend>,
}

#[pymethods]
impl PyMlxTargetBackend {
    #[new]
    #[pyo3(signature = (model_id, kv_cache_kind=None))]
    fn new(model_id: &str, kv_cache_kind: Option<String>) -> PyResult<Self> {
        let be = MlxTargetBackend::build(model_id, kv_cache_kind)?;
        Ok(Self {
            inner: Arc::new(be),
        })
    }

    fn info_json(&self) -> PyResult<String> {
        let i = self.inner.info();
        Ok(format!(
            "{{\"engine\":\"{}\",\"model_id\":\"{}\",\"device\":\"{}\",\"dtype\":\"{}\",\"kv_cache_type\":{}}}",
            i.engine,
            i.model_id,
            i.device,
            i.dtype,
            i.kv_cache_type
                .as_ref()
                .map(|s| format!("\"{s}\""))
                .unwrap_or_else(|| "null".to_string()),
        ))
    }
}

/// NullTargetBackend retained for tests / plumbing.
pub(crate) struct NullTargetBackend;

#[async_trait]
impl TargetBackend for NullTargetBackend {
    async fn forward(&self, _token_ids: &[u32]) -> Result<TargetOutput, String> {
        Ok(TargetOutput {
            logits: vec![0.0; 4],
            hidden: None,
            finished: false,
        })
    }
    fn info(&self) -> BackendInfo {
        BackendInfo {
            engine: "null".into(),
            model_id: "none".into(),
            device: "cpu".into(),
            dtype: "f32".into(),
            kv_cache_type: None,
        }
    }
}

/// Cheap clone wrapper around a PyMlxTargetBackend for engine use.
pub(crate) struct MlxTargetBackendClone(pub(crate) Arc<MlxTargetBackend>);

impl From<PyMlxTargetBackend> for MlxTargetBackendClone {
    fn from(p: PyMlxTargetBackend) -> Self {
        Self(p.inner)
    }
}

#[async_trait]
impl TargetBackend for MlxTargetBackendClone {
    async fn forward(&self, ids: &[u32]) -> Result<TargetOutput, String> {
        self.0.forward(ids).await
    }
    fn info(&self) -> BackendInfo {
        self.0.info()
    }
}
