// SPDX-License-Identifier: MIT OR Apache-2.0
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

mod artifact;
mod docker;
mod network;
mod seccomp;

pub use artifact::{Artifact, ArtifactBuilder};
pub use docker::DockerBackend;
pub use network::{NetworkMode, NetworkNamespace};
pub use seccomp::SeccompProfile;

/// The result of a sandbox execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    pub id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i64,
    pub artifact: Artifact,
    pub duration_ms: u64,
}

/// A sandbox configuration that defines an isolated execution environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sandbox {
    pub id: String,
    pub image: String,
    pub seccomp_profile: Option<SeccompProfile>,
    pub network_mode: NetworkMode,
    pub volume_mounts: Vec<String>,
    pub env: HashMap<String, String>,
    pub timeout: Duration,
}

impl Sandbox {
    /// Create a new sandbox with a unique ID.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            image: image.into(),
            seccomp_profile: Some(SeccompProfile::default()),
            network_mode: NetworkMode::Isolated,
            volume_mounts: Vec::new(),
            env: HashMap::new(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Set a custom seccomp profile.
    pub fn with_seccomp(mut self, profile: SeccompProfile) -> Self {
        self.seccomp_profile = Some(profile);
        self
    }

    /// Set the network mode.
    pub fn with_network(mut self, mode: NetworkMode) -> Self {
        self.network_mode = mode;
        self
    }

    /// Add a volume mount (format: `host:container` or `host:container:ro`).
    pub fn with_volume(mut self, mount: impl Into<String>) -> Self {
        self.volume_mounts.push(mount.into());
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the execution timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Run a command inside the sandbox and return the result.
    pub async fn run(&self, command: &str) -> Result<SandboxResult> {
        let backend = DockerBackend::new().await?;
        let start = std::time::Instant::now();

        // 1. Pull image by digest (if digest is present, otherwise pull by tag)
        backend.pull_image(&self.image).await?;

        // 2. Create container with seccomp + network
        let container_id = backend.create_container(self, command).await?;

        // 3. Start container
        backend.start_container(&container_id).await?;

        // 4. Exec command and capture output
        let (stdout, stderr, exit_code) = backend.exec_command(&container_id, command).await?;

        // 5. Compute artifact SHA-256 from stdout bytes
        let artifact = Artifact::builder()
            .sha256_from_bytes(stdout.as_bytes())
            .build();

        // 6. Stop + remove container
        backend.stop_container(&container_id).await?;
        backend.remove_container(&container_id).await?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SandboxResult {
            id: self.id.clone(),
            stdout,
            stderr,
            exit_code,
            artifact,
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_builder() {
        let sb = Sandbox::new("busybox@sha256:abc123")
            .with_seccomp(SeccompProfile::default())
            .with_network(NetworkMode::Isolated)
            .with_volume("/tmp:/data")
            .with_env("FOO", "bar")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(sb.image, "busybox@sha256:abc123");
        assert!(sb.seccomp_profile.is_some());
        assert_eq!(sb.network_mode, NetworkMode::Isolated);
        assert_eq!(sb.volume_mounts, vec!["/tmp:/data"]);
        assert_eq!(sb.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(sb.timeout, Duration::from_secs(60));
    }
}
