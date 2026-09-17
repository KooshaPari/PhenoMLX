// SPDX-License-Identifier: MIT OR Apache-2.0
//! Commit signing â€” GPG, SSH, or mock.

use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Signing backend trait.
#[async_trait::async_trait]
pub trait Signer: Send + Sync {
    /// Sign the commit object identified by `commit_sha` and return the
    /// signature data (or an updated commit SHA if the signer amends).
    async fn sign(&self, repo_root: &std::path::Path, commit_sha: &str) -> Result<String>;
}

/// GPG signer â€” uses `gpgme` when the `gpg` feature is enabled,
/// otherwise shells out to the `gpg` binary.
#[derive(Debug, Clone)]
pub struct GpgSigner {
    pub key_id: String,
}

#[async_trait::async_trait]
impl Signer for GpgSigner {
    async fn sign(&self, repo_root: &std::path::Path, commit_sha: &str) -> Result<String> {
        #[cfg(feature = "gpg")]
        {
            // gpgme::Context is !Send, so ALL gpgme work must be isolated inside
            // spawn_blocking. We fetch commit_text async first (it's a String = Send),
            // pass it into the closure by move, then await the async amend after
            // spawn_blocking completes (gpgme::Context is fully gone by then).
            let commit_text = get_commit_text(repo_root, commit_sha).await?;

            let key_id = self.key_id.clone();
            let signature_b64 = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
                use gpgme::{Context, Protocol};

                let mut ctx = Context::from_protocol(Protocol::OpenPgp)
                    .with_context(|| "failed to create GPG context")?;
                let key = ctx
                    .get_key(&key_id)
                    .with_context(|| format!("GPG key not found: {key_id}"))?;
                ctx.add_signer(&key)
                    .with_context(|| "failed to add signer to GPG context")?;

                let mut signature = Vec::new();
                ctx.sign_detached(commit_text.into_bytes(), &mut signature)
                    .with_context(|| "GPG sign failed")?;

                use base64::Engine as _;
                Ok(base64::engine::general_purpose::STANDARD.encode(&signature))
            })
            .await
            .context("spawn_blocking panicked")??;

            // gpgme::Context is gone; safe to .await again.
            let new_sha =
                amend_commit_with_gpg_signature(repo_root, commit_sha, &signature_b64).await?;
            return Ok(new_sha);
        }

        #[cfg(not(feature = "gpg"))]
        {
            // Shell out to gpg for detached signing.
            let commit_text = get_commit_text(repo_root, commit_sha).await?;
            let output = tokio::task::spawn_blocking({
                let key_id = self.key_id.clone();
                let commit_text = commit_text.clone();
                move || {
                    let mut child = Command::new("gpg")
                        .args(["--detach-sign", "--armor", "-u", &key_id, "-o", "-"])
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .with_context(|| "failed to spawn gpg")?;
                    use std::io::Write;
                    {
                        let stdin = child.stdin.take().context("gpg stdin unavailable")?;
                        let mut stdin = std::io::BufWriter::new(stdin);
                        stdin.write_all(commit_text.as_bytes())?;
                        stdin.flush()?;
                    } // drop stdin so gpg sees EOF
                    child
                        .wait_with_output()
                        .with_context(|| "gpg signing failed")
                }
            })
            .await
            .context("spawn_blocking failed")??;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("gpg sign failed: {}", stderr.trim());
            }

            let signature = String::from_utf8_lossy(&output.stdout);
            let new_sha =
                amend_commit_with_gpg_signature(repo_root, commit_sha, &signature).await?;
            Ok(new_sha)
        }
    }
}

/// SSH signer â€” uses `ssh-key` crate when the `ssh-sign` feature is enabled,
/// otherwise shells out to `ssh-keygen -Y sign`.
#[derive(Debug, Clone)]
pub struct SshSigner {
    pub key_path: std::path::PathBuf,
}

#[async_trait::async_trait]
impl Signer for SshSigner {
    async fn sign(&self, repo_root: &std::path::Path, commit_sha: &str) -> Result<String> {
        #[cfg(feature = "ssh-sign")]
        {
            use ssh_key::{HashAlg, LineEnding, PrivateKey};
            use std::fs;

            let pem = fs::read_to_string(&self.key_path)
                .with_context(|| format!("read SSH key: {}", self.key_path.display()))?;
            let private_key =
                PrivateKey::from_openssh(&pem).with_context(|| "parse SSH private key")?;

            let commit_text = get_commit_text(repo_root, commit_sha).await?;
            let sig = private_key
                .sign("git", HashAlg::Sha256, commit_text.as_bytes())
                .with_context(|| "SSH sign failed")?;
            // SshSig serializes to PEM-armored form which git expects for gpgsig headers.
            let signature_pem = sig
                .to_pem(LineEnding::LF)
                .with_context(|| "SSH sig to PEM failed")?;

            let new_sha =
                amend_commit_with_ssh_signature(repo_root, commit_sha, &signature_pem).await?;
            return Ok(new_sha);
        }

        #[cfg(not(feature = "ssh-sign"))]
        {
            // Shell out to ssh-keygen -Y sign
            let commit_text = get_commit_text(repo_root, commit_sha).await?;
            let output = tokio::task::spawn_blocking({
                let key_path = self.key_path.clone();
                let commit_text = commit_text.clone();
                move || {
                    let mut child = Command::new("ssh-keygen")
                        .args(["-Y", "sign", "-n", "git", "-f", &key_path.to_string_lossy()])
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .with_context(|| "failed to spawn ssh-keygen")?;
                    use std::io::Write;
                    {
                        let stdin = child.stdin.take().context("ssh-keygen stdin unavailable")?;
                        let mut stdin = std::io::BufWriter::new(stdin);
                        stdin.write_all(commit_text.as_bytes())?;
                        stdin.flush()?;
                    }
                    child
                        .wait_with_output()
                        .with_context(|| "ssh-keygen sign failed")
                }
            })
            .await
            .context("spawn_blocking failed")??;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("ssh-keygen sign failed: {}", stderr.trim());
            }

            let signature = String::from_utf8_lossy(&output.stdout);
            let new_sha =
                amend_commit_with_ssh_signature(repo_root, commit_sha, &signature).await?;
            Ok(new_sha)
        }
    }
}

/// Mock signer for tests â€” appends `[signed]` to the commit message.
#[derive(Debug, Clone, Default)]
pub struct MockSigner;

#[async_trait::async_trait]
impl Signer for MockSigner {
    async fn sign(&self, repo_root: &std::path::Path, commit_sha: &str) -> Result<String> {
        let new_msg = format!(
            "{}\n\n[signed]",
            get_commit_message(repo_root, commit_sha).await?
        );
        amend_commit_message(repo_root, &new_msg).await
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn get_commit_text(repo_root: &std::path::Path, commit_sha: &str) -> Result<String> {
    let repo_root = repo_root.to_path_buf();
    let commit_sha = commit_sha.to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("git")
            .args(["cat-file", "-p", &commit_sha])
            .current_dir(&repo_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("cat-file {commit_sha}"))
    })
    .await
    .context("spawn_blocking failed")??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git cat-file failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn get_commit_message(repo_root: &std::path::Path, commit_sha: &str) -> Result<String> {
    let repo_root = repo_root.to_path_buf();
    let commit_sha = commit_sha.to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("git")
            .args(["log", "-1", "--format=%B", &commit_sha])
            .current_dir(&repo_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("log {commit_sha}"))
    })
    .await
    .context("spawn_blocking failed")??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn amend_commit_message(repo_root: &std::path::Path, message: &str) -> Result<String> {
    let repo_root = repo_root.to_path_buf();
    let message = message.to_string();
    let root2 = repo_root.clone();
    let output = tokio::task::spawn_blocking({
        let message = message.clone();
        let repo_root = repo_root.clone();
        move || {
            Command::new("git")
                .args(["commit", "--amend", "-m", &message])
                .current_dir(&repo_root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .with_context(|| "git commit --amend")
        }
    })
    .await
    .context("spawn_blocking failed")??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git commit --amend failed: {}", stderr.trim());
    }

    // Return new HEAD.
    let output = tokio::task::spawn_blocking(move || {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root2)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| "rev-parse HEAD")
    })
    .await
    .context("spawn_blocking failed")??;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn amend_commit_with_gpg_signature(
    repo_root: &std::path::Path,
    commit_sha: &str,
    signature: &str,
) -> Result<String> {
    // Git's commit object format with a gpgsig header.
    let commit_text = get_commit_text(repo_root, commit_sha).await?;
    let lines: Vec<&str> = commit_text.lines().collect();
    // Insert gpgsig after the first blank line (or after tree line).
    let mut new_commit = String::new();
    let mut found_gpgsig = false;
    for line in &lines {
        if line.is_empty() && !found_gpgsig {
            new_commit.push_str("gpgsig ");
            new_commit.push_str(&signature.replace('\n', "\n "));
            new_commit.push('\n');
            found_gpgsig = true;
        }
        new_commit.push_str(line);
        new_commit.push('\n');
    }
    if !found_gpgsig {
        new_commit.push_str("gpgsig ");
        new_commit.push_str(&signature.replace('\n', "\n "));
        new_commit.push('\n');
    }

    write_commit_object(repo_root, &new_commit).await
}

async fn amend_commit_with_ssh_signature(
    repo_root: &std::path::Path,
    commit_sha: &str,
    signature: &str,
) -> Result<String> {
    // Git uses the same gpgsig header for SSH signatures in newer Git.
    // We reuse the same machinery.
    amend_commit_with_gpg_signature(repo_root, commit_sha, signature).await
}

async fn write_commit_object(repo_root: &std::path::Path, content: &str) -> Result<String> {
    let repo_root = repo_root.to_path_buf();
    let repo_root_for_reset = repo_root.clone();
    let content = content.to_string();
    let output = tokio::task::spawn_blocking({
        let content = content.clone();
        let repo_root = repo_root.clone();
        move || {
            let mut child = Command::new("git")
                .args(["hash-object", "-t", "commit", "--stdin", "-w"])
                .current_dir(&repo_root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .with_context(|| "git hash-object")?;
            use std::io::Write;
            {
                let stdin = child.stdin.take().context("stdin unavailable")?;
                let mut stdin = std::io::BufWriter::new(stdin);
                stdin.write_all(content.as_bytes())?;
                stdin.flush()?;
            }
            child
                .wait_with_output()
                .with_context(|| "git hash-object failed")
        }
    })
    .await
    .context("spawn_blocking failed")??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git hash-object failed: {}", stderr.trim());
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Reset HEAD to the new commit.
    let sha_for_reset = sha.clone();
    let _ = tokio::task::spawn_blocking(move || {
        Command::new("git")
            .args(["reset", "--soft", &sha_for_reset])
            .current_dir(&repo_root_for_reset)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    })
    .await;

    Ok(sha)
}
