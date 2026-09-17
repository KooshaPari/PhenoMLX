// SPDX-License-Identifier: MIT OR Apache-2.0
//! Audit log types — tamper-evident hash-chained entries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A reference to an evidence artifact in an audit entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub evidence_id: i64,
    pub fr_id: String,
}

/// A single entry in the tamper-evident audit chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub feature_id: i64,
    pub wp_id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub transition: String,
    pub evidence_refs: Vec<EvidenceRef>,
    pub prev_hash: [u8; 32],
    pub hash: [u8; 32],
    pub event_id: Option<i64>,
    pub archived_to: Option<String>,
}

/// A verified, hash-chained collection of audit entries.
pub struct AuditChain {
    pub entries: Vec<AuditEntry>,
}

impl AuditChain {
    /// Verify the hash chain is intact.  Returns `Err` with a description of
    /// the first broken link, or `Ok(())` if all hashes are consistent.
    pub fn verify_chain(&self) -> Result<(), String> {
        for (i, entry) in self.entries.iter().enumerate() {
            let computed = hash_entry(entry);
            if computed != entry.hash {
                return Err(format!(
                    "hash mismatch at entry index {i} (id={})",
                    entry.id
                ));
            }
            if i > 0 {
                let prev = &self.entries[i - 1];
                if entry.prev_hash != prev.hash {
                    return Err(format!(
                        "chain break between entries {} and {} (index {}-{})",
                        prev.id,
                        entry.id,
                        i - 1,
                        i
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Compute the SHA-256 hash of an audit entry (covers all mutable fields).
pub fn hash_entry(entry: &AuditEntry) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(entry.feature_id.to_be_bytes());
    if let Some(wp_id) = entry.wp_id {
        hasher.update(wp_id.to_be_bytes());
    }
    hasher.update(entry.timestamp.to_rfc3339().as_bytes());
    hasher.update(entry.actor.as_bytes());
    hasher.update(entry.transition.as_bytes());
    hasher.update(entry.prev_hash);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result[..]);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: i64, prev_hash: [u8; 32]) -> AuditEntry {
        let mut entry = AuditEntry {
            id,
            feature_id: 1,
            wp_id: None,
            timestamp: DateTime::from_timestamp(1_000_000 + id, 0).unwrap(),
            actor: "test-actor".to_string(),
            transition: "Draft->Active".to_string(),
            evidence_refs: vec![],
            prev_hash,
            hash: [0u8; 32],
            event_id: None,
            archived_to: None,
        };
        entry.hash = hash_entry(&entry);
        entry
    }

    #[test]
    fn hash_entry_is_deterministic() {
        let e = make_entry(1, [0u8; 32]);
        let h1 = hash_entry(&e);
        let h2 = hash_entry(&e);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_entry_changes_with_actor() {
        let mut e = make_entry(1, [0u8; 32]);
        let h1 = hash_entry(&e);
        e.actor = "different-actor".to_string();
        let h2 = hash_entry(&e);
        assert_ne!(h1, h2);
    }

    #[test]
    fn audit_chain_verify_valid_chain() {
        let entry1 = make_entry(1, [0u8; 32]);
        let entry2 = make_entry(2, entry1.hash);
        let chain = AuditChain {
            entries: vec![entry1, entry2],
        };
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn audit_chain_detects_tampered_hash() {
        let mut entry1 = make_entry(1, [0u8; 32]);
        entry1.hash = [0xff; 32]; // tamper
        let entry2 = make_entry(2, entry1.hash);
        let chain = AuditChain {
            entries: vec![entry1, entry2],
        };
        let result = chain.verify_chain();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hash mismatch"));
    }

    #[test]
    fn audit_chain_detects_broken_link() {
        let entry1 = make_entry(1, [0u8; 32]);
        let entry2 = make_entry(2, [0xab; 32]); // wrong prev_hash
        let chain = AuditChain {
            entries: vec![entry1, entry2],
        };
        let result = chain.verify_chain();
        assert!(result.is_err());
    }

    #[test]
    fn empty_audit_chain_verifies_ok() {
        let chain = AuditChain { entries: vec![] };
        assert!(chain.verify_chain().is_ok());
    }
}
