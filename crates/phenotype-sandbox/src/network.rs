// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NetworkMode {
    #[default]
    Isolated,
    Host,
    Bridged {
        bridge: String,
    },
}

impl NetworkMode {
    pub fn to_docker_string(&self) -> String {
        match self {
            NetworkMode::Isolated => "none".to_string(),
            NetworkMode::Host => "host".to_string(),
            NetworkMode::Bridged { bridge } => bridge.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNamespace {
    pub mode: NetworkMode,
}

impl NetworkNamespace {
    pub fn new(mode: NetworkMode) -> Self {
        Self { mode }
    }
    pub fn isolated() -> Self {
        Self::new(NetworkMode::Isolated)
    }
    pub fn host() -> Self {
        Self::new(NetworkMode::Host)
    }
    pub fn bridged(bridge: impl Into<String>) -> Self {
        Self::new(NetworkMode::Bridged {
            bridge: bridge.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_mode_to_docker_string() {
        assert_eq!(NetworkMode::Isolated.to_docker_string(), "none");
        assert_eq!(NetworkMode::Host.to_docker_string(), "host");
        assert_eq!(
            NetworkMode::Bridged {
                bridge: "my-bridge".to_string()
            }
            .to_docker_string(),
            "my-bridge"
        );
    }
}
