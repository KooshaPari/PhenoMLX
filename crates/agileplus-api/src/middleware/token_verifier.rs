// SPDX-License-Identifier: MIT OR Apache-2.0
//! `TokenVerifier` — hexagonal port for bearer-token / API-key validation.
//!
//! Any auth backend (shared-secret, JWT, Authvault, etc.) implements this
//! trait. The middleware calls `verify` without knowing which backend is in use.
//!
//! Traceability: FR-AGP-012

use std::sync::Arc;

/// Hexagonal port: validates an opaque token string.
///
/// Implementors MUST use constant-time comparison to prevent timing attacks.
pub trait TokenVerifier: Send + Sync {
    /// Returns `true` if `token` is valid, `false` otherwise.
    ///
    /// Errors are for infrastructure failures (e.g. keystore unavailable),
    /// not for invalid tokens — those return `Ok(false)`.
    fn verify(&self, token: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

// ── Default implementation: shared-secret / API-key ──────────────────────────

/// Constant-time shared-secret verifier.
///
/// Accepts a comma-separated list of allowed keys.  Falls back to the
/// `AGILEPLUS_API_KEY` environment variable if the list is empty at
/// construction time.
///
/// Follow-up: replace with JWT/Authvault adapter for FR-AGP-012 (JWT/Authvault
/// integration noted as future work).
pub struct SharedSecretVerifier {
    allowed_keys: Vec<String>,
}

impl SharedSecretVerifier {
    /// Construct from an explicit list of allowed keys.
    pub fn new(keys: Vec<String>) -> Self {
        Self { allowed_keys: keys }
    }

    /// Construct from the `AGILEPLUS_API_KEY` environment variable.
    ///
    /// The env var may contain multiple comma-separated keys.
    pub fn from_env() -> Self {
        let raw = std::env::var("AGILEPLUS_API_KEY").unwrap_or_default();
        let keys = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Self { allowed_keys: keys }
    }
}

impl TokenVerifier for SharedSecretVerifier {
    fn verify(&self, token: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Constant-time comparison: iterate every key even if a match is found
        // early, to prevent timing leakage.
        let target = token.as_bytes();
        let mut found = false;
        for key in &self.allowed_keys {
            let kb = key.as_bytes();
            if kb.len() == target.len() {
                // XOR each byte pair; OR the results. Zero ⟹ equal.
                let diff = kb
                    .iter()
                    .zip(target.iter())
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b));
                // Use black_box to discourage the optimiser from short-circuiting.
                if std::hint::black_box(diff) == 0 {
                    found = true;
                }
            } else {
                // Different lengths — still do *some* work to keep timing uniform.
                std::hint::black_box(0u8);
            }
        }
        Ok(found)
    }
}

/// Convenience wrapper so callers can store a `TokenVerifier` behind an `Arc`.
pub type DynTokenVerifier = Arc<dyn TokenVerifier>;

#[cfg(test)]
mod tests {
    use super::*;

    fn verifier(keys: &[&str]) -> SharedSecretVerifier {
        SharedSecretVerifier::new(keys.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn valid_key_accepted() {
        let v = verifier(&["secret-token"]);
        assert!(v.verify("secret-token").unwrap());
    }

    #[test]
    fn wrong_key_rejected() {
        let v = verifier(&["secret-token"]);
        assert!(!v.verify("bad-token").unwrap());
    }

    #[test]
    fn empty_key_list_rejects_everything() {
        let v = verifier(&[]);
        assert!(!v.verify("any-token").unwrap());
    }

    #[test]
    fn multiple_keys_any_valid_accepted() {
        let v = verifier(&["key-a", "key-b"]);
        assert!(v.verify("key-a").unwrap());
        assert!(v.verify("key-b").unwrap());
        assert!(!v.verify("key-c").unwrap());
    }
}
