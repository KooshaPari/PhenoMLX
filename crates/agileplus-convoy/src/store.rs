// SPDX-License-Identifier: MIT OR Apache-2.0
//! In-memory convoy store.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::{Convoy, ConvoyStatus};

/// In-memory store for convoys.
#[derive(Debug, Default, Clone)]
pub struct ConvoyStore {
    convoys: HashMap<Uuid, Convoy>,
}

impl ConvoyStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a convoy.
    pub fn add(&mut self, convoy: Convoy) {
        self.convoys.insert(convoy.id, convoy);
    }

    /// Get a convoy by id.
    pub fn get(&self, id: Uuid) -> Option<Convoy> {
        self.convoys.get(&id).cloned()
    }

    /// Update a convoy in place.
    pub fn update(&mut self, convoy: Convoy) -> Result<()> {
        if !self.convoys.contains_key(&convoy.id) {
            return Err(anyhow!("convoy not found: {}", convoy.id));
        }
        self.convoys.insert(convoy.id, convoy);
        Ok(())
    }

    /// List convoys filtered by status.
    pub fn list_by_status(&self, status: ConvoyStatus) -> Vec<Convoy> {
        self.convoys
            .values()
            .filter(|c| c.status == status)
            .cloned()
            .collect()
    }

    /// All convoys.
    pub fn all(&self) -> Vec<Convoy> {
        self.convoys.values().cloned().collect()
    }
}
