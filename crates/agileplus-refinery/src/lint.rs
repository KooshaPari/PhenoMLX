// SPDX-License-Identifier: MIT OR Apache-2.0
//! Lint runner — cargo check, clippy, fmt, test.

use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Per-check outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintCheck {
    pub name: String,
    pub passed: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Aggregated lint result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintResult {
    pub checks: Vec<LintCheck>,
    pub overall_pass: bool,
}

/// Lint runner.
#[derive(Debug, Clone)]
pub struct Lint;

impl Lint {
    /// Run the full lint suite in the given working directory.
    pub async fn run(&self, working_dir: &std::path::Path) -> Result<LintResult> {
        let mut checks = Vec::with_capacity(4);

        checks.push(Self::run_check("cargo check", working_dir, &["check"]).await?);
        checks.push(
            Self::run_check(
                "cargo clippy",
                working_dir,
                &["clippy", "--", "-D", "warnings"],
            )
            .await?,
        );
        checks.push(Self::run_check("cargo fmt", working_dir, &["fmt", "--", "--check"]).await?);
        checks.push(Self::run_check("cargo test", working_dir, &["test"]).await?);

        let overall_pass = checks.iter().all(|c| c.passed);
        Ok(LintResult {
            checks,
            overall_pass,
        })
    }

    async fn run_check(name: &str, cwd: &std::path::Path, args: &[&str]) -> Result<LintCheck> {
        let output = tokio::task::spawn_blocking({
            let cwd = cwd.to_path_buf();
            let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            let name = name.to_string();
            move || {
                Command::new("cargo")
                    .args(&args)
                    .current_dir(&cwd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .with_context(|| format!("{name}: failed to spawn cargo"))
            }
        })
        .await
        .context("spawn_blocking failed")??;

        let passed = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        Ok(LintCheck {
            name: name.to_string(),
            passed,
            stdout,
            stderr,
        })
    }
}
