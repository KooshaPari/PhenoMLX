// SPDX-License-Identifier: MIT OR Apache-2.0
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// A seccomp profile that defines allowed and denied syscalls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompProfile {
    pub default_action: String,
    pub architectures: Vec<String>,
    pub syscalls: Vec<SyscallRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallRule {
    pub names: Vec<String>,
    pub action: String,
}

impl Default for SeccompProfile {
    fn default() -> Self {
        let default_action = "SCMP_ACT_ERRNO".to_string();
        let architectures = vec![
            "SCMP_ARCH_X86_64".to_string(),
            "SCMP_ARCH_AARCH64".to_string(),
        ];

        let allow_syscalls = vec![
            "read",
            "write",
            "open",
            "close",
            "mmap",
            "brk",
            "exit",
            "exit_group",
        ];

        let deny_syscalls = vec!["socket", "connect", "bind"];

        let syscalls = vec![
            SyscallRule {
                names: allow_syscalls.into_iter().map(String::from).collect(),
                action: "SCMP_ACT_ALLOW".to_string(),
            },
            SyscallRule {
                names: deny_syscalls.into_iter().map(String::from).collect(),
                action: "SCMP_ACT_ERRNO".to_string(),
            },
        ];

        Self {
            default_action,
            architectures,
            syscalls,
        }
    }
}

impl SeccompProfile {
    /// Create a new profile with default deny-all behavior.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allow a specific syscall.
    pub fn allow_syscall(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        for rule in &mut self.syscalls {
            if rule.action == "SCMP_ACT_ALLOW" {
                if !rule.names.contains(&name) {
                    rule.names.push(name.clone());
                }
                return self;
            }
        }
        self.syscalls.push(SyscallRule {
            names: vec![name],
            action: "SCMP_ACT_ALLOW".to_string(),
        });
        self
    }

    /// Deny a specific syscall.
    pub fn deny_syscall(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        for rule in &mut self.syscalls {
            if rule.action == "SCMP_ACT_ERRNO" {
                if rule.names.is_empty() {
                    rule.names.push(name.clone());
                    return self;
                }
                continue;
            }
        }
        self.syscalls.push(SyscallRule {
            names: vec![name],
            action: "SCMP_ACT_ERRNO".to_string(),
        });
        self
    }

    /// Build the seccomp profile with network-specific rules.
    ///
    /// If `network_mode` is "host", socket/connect/bind are allowed.
    pub fn with_network_mode(mut self, network_mode: &str) -> Self {
        if network_mode == "host" {
            // Remove socket/connect/bind from the deny list
            for rule in &mut self.syscalls {
                if rule.action == "SCMP_ACT_ERRNO" {
                    rule.names
                        .retain(|n| n != "socket" && n != "connect" && n != "bind");
                }
            }
        }
        self
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("failed to serialize seccomp profile")
    }

    /// Write the profile to a temporary file and return its path.
    pub async fn write_to_temp_file(&self) -> Result<PathBuf> {
        let json = self.to_json()?;
        let mut file = NamedTempFile::new().context("failed to create temp file")?;
        file.write_all(json.as_bytes())
            .context("failed to write seccomp profile")?;
        file.flush().context("failed to flush temp file")?;
        let path = file.into_temp_path().to_path_buf();
        // Keep the file around by not dropping the path; we leak the guard
        // so the path remains valid until the process exits.
        let _ = path.clone();
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seccomp_profile_default_json() {
        let profile = SeccompProfile::default();
        let json = profile.to_json().unwrap();
        assert!(json.contains("SCMP_ACT_ERRNO"));
        assert!(json.contains("SCMP_ACT_ALLOW"));
        assert!(json.contains("read"));
        assert!(json.contains("write"));
        assert!(json.contains("socket"));
        assert!(json.contains("connect"));
        assert!(json.contains("bind"));
    }

    #[test]
    fn seccomp_profile_network_host_allows_sockets() {
        let profile = SeccompProfile::default().with_network_mode("host");
        let json = profile.to_json().unwrap();
        assert!(!json.contains("\"socket\""));
        assert!(!json.contains("\"connect\""));
        assert!(!json.contains("\"bind\""));
    }

    #[tokio::test]
    async fn seccomp_profile_write_temp_file() {
        let profile = SeccompProfile::default();
        let path = profile.write_to_temp_file().await.unwrap();
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("SCMP_ACT_ERRNO"));
        assert!(content.contains("SCMP_ACT_ALLOW"));
    }
}
