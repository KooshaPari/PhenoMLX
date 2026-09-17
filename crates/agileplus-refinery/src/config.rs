// SPDX-License-Identifier: MIT OR Apache-2.0
//! Refinery pipeline configuration.

use serde::{Deserialize, Serialize};

/// Configuration knobs for the post-processing pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefineryConfig {
    /// Whether to squash the source branch into the target branch.
    pub squash: bool,
    /// Whether to GPG/SSH-sign the resulting commit.
    pub sign: bool,
    /// Whether to create an annotated tag after the signed commit.
    pub tag: bool,
    /// Whether to run the lint suite (`cargo check`, `clippy`, `fmt`, `test`).
    pub lint: bool,
    /// GPG key ID for commit signing. If `sign` is true and this is set,
    /// GPG signing is preferred over SSH signing.
    pub gpg_key_id: Option<String>,
    /// Path to an SSH private key for commit signing. Used when `sign` is
    /// true and `gpg_key_id` is `None`.
    pub ssh_key_path: Option<String>,
}
