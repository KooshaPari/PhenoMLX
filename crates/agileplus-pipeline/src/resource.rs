// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};

/// Resource limits for pipeline execution.
/// Passed to the executor for future k8s integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub cpus: u32,
    pub mem: String,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpus: 1,
            mem: "512M".to_string(),
        }
    }
}

impl ResourceLimits {
    pub fn new(cpus: u32, mem: impl Into<String>) -> Self {
        Self {
            cpus,
            mem: mem.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpus, 1);
        assert_eq!(limits.mem, "512M");
    }

    #[test]
    fn custom_limits() {
        let limits = ResourceLimits::new(4, "2G");
        assert_eq!(limits.cpus, 4);
        assert_eq!(limits.mem, "2G");
    }
}
