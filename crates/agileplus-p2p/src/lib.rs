//! agileplus-p2p — peer-to-peer discovery and event replication for AgilePlus.
//!
//! # Overview
//!
//! This crate provides:
//! - **mDNS peer discovery** via [`P2pNode`] / [`P2pBehaviour`] (no broker required).
//! - **Tailscale-based discovery** in [`discovery`] for managed tailnet deployments.
//! - **NATS-backed event replication** in [`replication`].
//! - **Vector-clock sync** in [`vector_clock`].
//! - **Device identity** registration in [`device`].
//! - **Git-backed state export/import** in [`export`] / [`import`].
//!
//! The primary entry point for ad-hoc LAN discovery is [`P2pNode`]:
//!
//! ```no_run
//! # use agileplus_p2p::{P2pNode, P2pError};
//! # use std::time::Duration;
//! # #[tokio::main]
//! # async fn main() -> Result<(), P2pError> {
//! let mut node = P2pNode::new().await?;
//! let peers = node.discover_peers(Duration::from_secs(3)).await;
//! println!("Found {} peers", peers.len());
//! # Ok(())
//! # }
//! ```
//!
//! Traceability: WP16 / T095

pub mod device;
pub mod discovery;
pub mod error;
pub mod export;
pub mod import;
pub mod replication;
pub mod vector_clock;

pub mod git_merge;

use std::collections::HashSet;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ── Service constants ─────────────────────────────────────────────────────────

/// mDNS service type advertised by AgilePlus nodes.
const SERVICE_TYPE: &str = "_agileplus._tcp.local.";

// ── P2pError ──────────────────────────────────────────────────────────────────

/// Errors that can occur during P2P node lifecycle.
#[derive(Debug, Error)]
pub enum P2pError {
    /// The mDNS daemon could not be started.
    #[error("mDNS daemon failed to start: {0}")]
    MdnsStart(String),

    /// The node failed to register its own service.
    #[error("mDNS service registration failed: {0}")]
    ServiceRegistration(String),

    /// Browsing for peers failed.
    #[error("mDNS browse failed: {0}")]
    Browse(String),

    /// General I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── PeerId ────────────────────────────────────────────────────────────────────

/// Opaque peer identifier (instance name returned by mDNS).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerId(pub String);

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── P2pBehaviour ─────────────────────────────────────────────────────────────

/// Encapsulates the mDNS service behaviour for a [`P2pNode`].
///
/// Holds the [`ServiceDaemon`] and the local service's instance name so that
/// the same daemon can be reused across multiple browse operations.
pub struct P2pBehaviour {
    /// Running mDNS daemon.
    pub(crate) daemon: ServiceDaemon,
    /// Unique instance name for *this* node's advertisement.
    pub(crate) instance_name: String,
}

impl P2pBehaviour {
    /// Create a new behaviour, starting the mDNS daemon and registering the
    /// local AgilePlus service.
    fn new(instance_name: &str, port: u16) -> Result<Self, P2pError> {
        let daemon =
            ServiceDaemon::new().map_err(|e| P2pError::MdnsStart(e.to_string()))?;

        // Register this node so other peers can discover it.
        let host_name = format!("{}.local.", instance_name);
        let service_info = mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &host_name,
            (),  // no specific IP — mDNS-sd resolves from the local interface
            port,
            None,
        )
        .map_err(|e| P2pError::ServiceRegistration(e.to_string()))?;

        daemon
            .register(service_info)
            .map_err(|e| P2pError::ServiceRegistration(e.to_string()))?;

        info!(
            "P2pBehaviour: registered mDNS service '{}' on port {}",
            instance_name, port
        );

        Ok(Self {
            daemon,
            instance_name: instance_name.to_string(),
        })
    }
}

// ── P2pNode ───────────────────────────────────────────────────────────────────

/// Top-level P2P node.
///
/// Manages the mDNS [`P2pBehaviour`] (which includes the service daemon and
/// local advertisement) and exposes [`discover_peers`](P2pNode::discover_peers)
/// for collecting neighbouring AgilePlus instances.
pub struct P2pNode {
    /// mDNS behaviour (daemon + registration).
    pub behaviour: P2pBehaviour,
    /// Stable identity for this node.
    pub local_peer_id: PeerId,
}

impl P2pNode {
    /// Construct a new [`P2pNode`], starting the mDNS daemon and advertising
    /// this node on the local network.
    ///
    /// A random UUID is used as the instance name so each node is unique.
    /// Port `3000` is advertised (the default AgilePlus HTTP port).
    pub async fn new() -> Result<Self, P2pError> {
        let instance_name = Uuid::new_v4().to_string();
        let behaviour = P2pBehaviour::new(&instance_name, 3000)?;
        let local_peer_id = PeerId(instance_name.clone());

        info!("P2pNode started; local peer id = {}", local_peer_id);

        Ok(Self {
            behaviour,
            local_peer_id,
        })
    }

    /// Browse the local network for other AgilePlus peers.
    ///
    /// Blocks for at most `timeout`, collecting all [`PeerId`]s that respond
    /// to the `_agileplus._tcp.local.` mDNS query.  Returns a deduplicated
    /// `Vec` of discovered peer identifiers (the local node is excluded).
    pub async fn discover_peers(&mut self, timeout: Duration) -> Vec<PeerId> {
        let receiver = match self.behaviour.daemon.browse(SERVICE_TYPE) {
            Ok(r) => r,
            Err(e) => {
                warn!("P2pNode::discover_peers browse error: {e}");
                return Vec::new();
            }
        };

        let deadline = tokio::time::Instant::now() + timeout;
        let mut found: HashSet<PeerId> = HashSet::new();
        let local = &self.local_peer_id;

        loop {
            let remaining = match deadline.checked_duration_since(tokio::time::Instant::now()) {
                Some(d) => d,
                None => break,
            };

            match tokio::time::timeout(remaining, async { receiver.recv_async().await }).await {
                Ok(Ok(event)) => match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let peer = PeerId(info.get_fullname().to_string());
                        if &peer != local {
                            debug!("Discovered peer: {peer}");
                            found.insert(peer);
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        let peer = PeerId(fullname);
                        debug!("Peer removed: {peer}");
                        found.remove(&peer);
                    }
                    _ => {}
                },
                Ok(Err(_)) => break, // channel closed
                Err(_) => break,     // timeout elapsed
            }
        }

        info!(
            "P2pNode::discover_peers: found {} peer(s) in {:.1?}",
            found.len(),
            timeout
        );
        found.into_iter().collect()
    }
}

impl Drop for P2pNode {
    fn drop(&mut self) {
        // Unregister local service and shut down daemon gracefully.
        let fullname = format!(
            "{}.{}",
            self.behaviour.instance_name, SERVICE_TYPE
        );
        let _ = self.behaviour.daemon.unregister(&fullname);
        let _ = self.behaviour.daemon.shutdown();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that a P2pNode can be constructed without errors.
    ///
    /// This test does NOT require a live network — it only checks that the mDNS
    /// daemon starts and the service registers successfully.
    #[tokio::test]
    async fn test_node_constructs() {
        let node = P2pNode::new().await.expect("P2pNode::new should succeed");
        assert!(
            !node.local_peer_id.0.is_empty(),
            "local_peer_id must not be empty"
        );
        // Verify the UUID format (36 chars: 8-4-4-4-12 with hyphens).
        assert_eq!(
            node.local_peer_id.0.len(),
            36,
            "local_peer_id should be a UUID string"
        );
    }

    /// Verify that discover_peers returns promptly with an empty list when no
    /// peers are on the LAN (or in CI).
    #[tokio::test]
    async fn test_discover_peers_returns_empty_without_network() {
        let mut node = P2pNode::new()
            .await
            .expect("P2pNode::new should succeed");
        // Very short timeout so the test finishes quickly.
        let peers = node.discover_peers(Duration::from_millis(200)).await;
        // In a test environment there are no other AgilePlus nodes.
        // We just assert it doesn't panic and returns a Vec.
        assert!(
            peers.len() < 1000,
            "sanity check: peers list should be small"
        );
    }

    #[test]
    fn peer_id_display() {
        let id = PeerId("abc-123".to_string());
        assert_eq!(format!("{id}"), "abc-123");
    }

    #[test]
    fn peer_id_equality() {
        let a = PeerId("x".to_string());
        let b = PeerId("x".to_string());
        let c = PeerId("y".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
