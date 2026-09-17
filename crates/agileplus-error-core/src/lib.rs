// SPDX-License-Identifier: MIT OR Apache-2.0
//! Canonical error types for the local AgilePlus workspace.
//!
//! This crate replaces the previous sibling-repo dependency on
//! `phenotype-error-core` with an in-repo, workspace-owned boundary.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    NotFound,
    AlreadyExists,
    ValidationError,
    NotImplemented,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub code: ErrorCode,
    pub message: String,
}

impl ErrorEnvelope {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
