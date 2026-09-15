use crate::domain::Cell;

pub(crate) fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        s[..n].to_string()
    }
}

pub(crate) fn evals_root() -> String {
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|wd| format!("{}/..", wd.display()))
            .unwrap_or_default(),
        std::env::current_dir()
            .ok()
            .map(|wd| wd.display().to_string())
            .unwrap_or_default(),
        ".".into(),
        "..".into(),
    ];

    for root in &candidates {
        let path = format!("{}/scripts/evals/run_langfuse_evaluators.py", root);
        if std::path::Path::new(&path).exists() {
            return root.clone();
        }
    }
    "..".into()
}

pub(crate) fn evals_python() -> String {
    if let Ok(v) = std::env::var("EVALS_PYTHON") {
        let trimmed = v.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let root = evals_root();
    let venv_py = format!("{}/.venv-evals/bin/python", root);
    if std::path::Path::new(&venv_py).exists() {
        return venv_py;
    }
    "python3".into()
}

pub(crate) fn load_current_cells() -> Result<Vec<Cell>, Box<dyn std::error::Error>> {
    let data_path = std::env::var("BENCH_DATA_PATH")
        .or_else(|_| std::env::var("DATA_PATH"))
        .unwrap_or_default();
    if data_path.is_empty() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read(&data_path)?;
    let data: crate::domain::ResultsData = serde_json::from_slice(&raw)?;
    Ok(data.cells)
}
