// SPDX-License-Identifier: MIT OR Apache-2.0
//! In-memory hook registry.

use std::collections::HashMap;

use crate::Hook;

/// Registry of hooks keyed by id.
#[derive(Debug, Default, Clone)]
pub struct HookRegistry {
    hooks: HashMap<String, Hook>,
}

impl HookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a hook. Returns any previous hook with the same id.
    pub fn register(&mut self, hook: Hook) -> Option<Hook> {
        self.hooks.insert(hook.id.clone(), hook)
    }

    /// Unregister a hook by id. Returns the removed hook if present.
    pub fn unregister(&mut self, id: &str) -> Option<Hook> {
        self.hooks.remove(id)
    }

    /// List all registered hooks.
    pub fn list(&self) -> Vec<Hook> {
        self.hooks.values().cloned().collect()
    }

    /// Get a hook by id.
    pub fn get(&self, id: &str) -> Option<Hook> {
        self.hooks.get(id).cloned()
    }

    /// Count of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}
