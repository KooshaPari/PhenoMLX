//! # AgilePlus Governance System
//!
//! A comprehensive governance system for the AgilePlus platform providing:
//! - **Release Channel Governance**: 5-tier release model (alpha → canary → beta → rc → prod)
//! - **Policy Enforcement**: Rule-based policy checks for promotions and operations
//! - **Audit Logging**: Complete audit trail of all governance actions
//! - **Rate Limiting**: Protection against abuse
//!
//! ## Release Channels
//!
//! | Channel | Version Suffix | Description |
//! |---------|----------------|-------------|
//! | alpha   | `-alpha.N`     | Experimental features |
//! | canary  | `-canary.N`    | Early access, unstable |
//! | beta    | `-beta.N`      | Public testing |
//! | rc      | `-rc.N`        | Release candidate |
//! | prod    | (none)         | Production stable |
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use agileplus_governance::{GovernanceClient, PolicyCheck, PolicyContext, ReleaseChannel};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let governance = GovernanceClient::with_defaults().await?;
//!
//! // Check if a promotion is allowed
//! let check = PolicyCheck {
//!     resource: "my-crate".to_string(),
//!     action: "promote".to_string(),
//!     context: PolicyContext::new()
//!         .with_channel(ReleaseChannel::Canary),
//! };
//!
//! let result = governance.check_policy(check).await?;
//! println!("Promotion allowed: {}", result.allowed);
//! # Ok(())
//! # }
//! ```

pub mod audit;
pub mod channel;
pub mod client;
pub mod code_scanner;
pub mod scoring_engine;
pub mod config;
pub mod error;
pub mod policy;
pub mod rate_limiter;
pub mod rubric;
pub mod types;

pub use audit::AuditLogger;
pub use channel::{ChannelMetadata, PromotionRequest, ReleaseChannel};
pub use client::GovernanceClient;
pub use config::GovernanceConfig;
pub use error::{GovernanceError, Result};
pub use policy::{PolicyCheck, PolicyContext, PolicyEngine, PolicyResult};
pub use rate_limiter::RateLimiter;
pub use code_scanner::{scan_repo, EvidenceItem, RepoScan};
pub use scoring_engine::{evaluate, render_markdown, ClusterScore, PillarScore, ScoreReport};
pub use rubric::{Pillar, RubricCatalog, ScoringSpec, SubPillar};
pub use types::*;

// Re-export commonly used types
pub use channel::ReleaseChannel as Channel;
