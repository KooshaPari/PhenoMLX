// SPDX-License-Identifier: MIT OR Apache-2.0
use anyhow::{Context, Result};
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::image::CreateImageOptions;
use bollard::models::HostConfig;
use bollard::Docker;
use futures::StreamExt;
use tracing::{debug, info, warn};

use crate::Sandbox;

/// Docker backend for managing sandbox containers.
pub struct DockerBackend {
    client: Docker,
}

impl DockerBackend {
    /// Connect to the local Docker daemon.
    pub async fn new() -> Result<Self> {
        let client = Docker::connect_with_local_defaults()
            .context("failed to connect to local Docker daemon")?;
        Ok(Self { client })
    }

    /// Pull an image by digest or tag.
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        info!(image, "pulling Docker image");
        let options = Some(CreateImageOptions {
            from_image: image.to_string(),
            ..Default::default()
        });

        let mut stream = self.client.create_image(options, None, None);
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(status) => {
                    if let Some(error) = status.error {
                        return Err(anyhow::anyhow!("Docker pull error: {error}"));
                    }
                    debug!(?status, "pull status");
                }
                Err(e) => {
                    warn!(error = %e, "pull stream error");
                }
            }
        }
        Ok(())
    }

    /// Create a container with the sandbox configuration.
    pub async fn create_container(&self, sandbox: &Sandbox, command: &str) -> Result<String> {
        info!(sandbox.id, "creating container");

        let seccomp_opt = if let Some(ref profile) = sandbox.seccomp_profile {
            let path = profile.write_to_temp_file().await?;
            Some(format!("seccomp={}", path.display()))
        } else {
            None
        };

        let mut security_opt = Vec::new();
        if let Some(opt) = seccomp_opt {
            security_opt.push(opt);
        }

        let network_mode = sandbox.network_mode.to_docker_string();
        let binds = if sandbox.volume_mounts.is_empty() {
            None
        } else {
            Some(sandbox.volume_mounts.clone())
        };

        let env: Vec<String> = sandbox
            .env
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();

        let cmd = shell_words::split(command)?;

        let host_config = HostConfig {
            network_mode: Some(network_mode),
            binds,
            security_opt: Some(security_opt),
            ..Default::default()
        };

        let config = Config {
            image: Some(sandbox.image.clone()),
            cmd: Some(cmd),
            env: Some(env),
            host_config: Some(host_config),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: format!("phenotype-sandbox-{}", sandbox.id),
            ..Default::default()
        };

        let result = self
            .client
            .create_container(Some(options), config)
            .await
            .context("failed to create container")?;

        Ok(result.id)
    }

    /// Start an existing container.
    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        info!(container_id, "starting container");
        self.client
            .start_container::<String>(container_id, None)
            .await
            .context("failed to start container")?;
        Ok(())
    }

    /// Execute a command inside a running container and capture stdout/stderr.
    pub async fn exec_command(
        &self,
        container_id: &str,
        command: &str,
    ) -> Result<(String, String, i64)> {
        info!(container_id, command, "executing command in container");

        let cmd = shell_words::split(command)?;

        let exec_options = CreateExecOptions {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            cmd: Some(cmd),
            ..Default::default()
        };

        let exec = self
            .client
            .create_exec(container_id, exec_options)
            .await
            .context("failed to create exec")?;

        let start_options = StartExecOptions {
            ..Default::default()
        };

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exec_result = self
            .client
            .start_exec(&exec.id, Some(start_options))
            .await?;

        if let bollard::exec::StartExecResults::Attached { mut output, .. } = exec_result {
            while let Some(chunk) = output.next().await {
                match chunk {
                    Ok(bollard::container::LogOutput::StdOut { message }) => {
                        stdout.extend_from_slice(&message);
                    }
                    Ok(bollard::container::LogOutput::StdErr { message }) => {
                        stderr.extend_from_slice(&message);
                    }
                    Ok(bollard::container::LogOutput::Console { message }) => {
                        stdout.extend_from_slice(&message);
                    }
                    Err(e) => {
                        error!(error = %e, "exec stream error");
                    }
                    _ => {}
                }
            }
        }

        if let bollard::exec::StartExecResults::Attached { mut output, .. } = exec_result {
            while let Some(chunk) = output.next().await {
                match chunk {
                    Ok(bollard::container::LogOutput::StdOut { message }) => {
                        stdout.extend_from_slice(&message);
                    }
                    Ok(bollard::container::LogOutput::StdErr { message }) => {
                        stderr.extend_from_slice(&message);
                    }
                    Ok(bollard::container::LogOutput::Console { message }) => {
                        stdout.extend_from_slice(&message);
                    }
                    Err(e) => {
                        error!(error = %e, "exec stream error");
                    }
                    _ => {}
                }
                // Inspect exec to get exit code
                let inspect = self.client.inspect_exec(&exec.id).await?;
                inspect.exit_code.unwrap_or(-1)
            }
            StartExecResults::Detached => 0,
        };

        let stdout = String::from_utf8_lossy(&stdout).to_string();
        let stderr = String::from_utf8_lossy(&stderr).to_string();

        Ok((stdout, stderr, exit_code))
    }

    /// Stop a container.
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        info!(container_id, "stopping container");
        let options = StopContainerOptions { t: 10 };
        if let Err(e) = self
            .client
            .stop_container(container_id, Some(options))
            .await
        {
            warn!(error = %e, "stop container failed (container may already be stopped)");
        }
        Ok(())
    }

    /// Remove a container.
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        info!(container_id, "removing container");
        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        if let Err(e) = self
            .client
            .remove_container(container_id, Some(options))
            .await
        {
            warn!(error = %e, "remove container failed (container may already be removed)");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn docker_connect() {
        // Skip if Docker is not available.
        if DockerBackend::new().await.is_err() {
            eprintln!("Docker not available, skipping integration test");
            return;
        }
    }
}
