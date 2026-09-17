//! API key generation and lifecycle management.
//!
//! On first startup, if no API key exists in the credential store, a new
//! 32-byte random key is generated, base64url-encoded as the plaintext key,
//! and only its SHA-256 hash is stored for validation. The plaintext key is
//! never persisted by AgilePlus.
//!
//! Traceability: WP11-T064

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};

use agileplus_domain::credentials::{CredentialStore, format_api_key_hash, keys};

/// Prefix that identifies an AgilePlus API key.
const KEY_PREFIX: &str = "agp_";

/// Generate a new API key: 32 random bytes → base64url-encoded plaintext.
///
/// Returns the plaintext key (to be shown to the user once).
pub fn generate_plaintext_key() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("{}{}", KEY_PREFIX, URL_SAFE_NO_PAD.encode(bytes))
}

/// Hash a plaintext API key using SHA-256.
pub fn hash_key(plaintext: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    hasher.finalize().into()
}

/// Import an operator-supplied key, replacing any legacy plaintext entry with
/// its non-reversible representation. Operators retain the source key in their
/// secret manager; AgilePlus never persists or reprints it.
pub fn import_api_key(
    creds: &dyn CredentialStore,
    plaintext: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if plaintext.trim().is_empty() {
        return Err("AGILEPLUS_API_KEY must not be empty".into());
    }
    creds.set("agileplus", keys::API_KEYS, &format_api_key_hash(plaintext))?;
    Ok(())
}

/// Ensure an API key exists in the credential store.
///
/// If no key is found, generates a new one and stores only its hash. The
/// generated plaintext is deliberately never written to disk.
///
/// Returns `true` if a new key was generated, `false` if one already existed.
pub async fn ensure_api_key(
    creds: &dyn CredentialStore,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Check if a key already exists.
    let existing = creds.get("agileplus", keys::API_KEYS);
    if let Ok(val) = existing {
        if !val.trim().is_empty() {
            return Ok(false);
        }
    }

    // Generate new key.
    let plaintext = generate_plaintext_key();

    creds.set(
        "agileplus",
        keys::API_KEYS,
        &format_api_key_hash(&plaintext),
    )?;

    // Log key metadata securely (never log the actual key)
    tracing::info!("AgilePlus API initialized with a non-reversible API key hash");
    // Show only first 8 chars + "..." for operator confirmation
    let masked_key = if plaintext.len() > 8 {
        format!("{}...", &plaintext[..8])
    } else {
        "[key too short]".to_string()
    };
    println!("AgilePlus API initialized.");
    println!("API Key (masked): {}", masked_key);
    println!("Provide an operator-managed API key through secure deployment configuration.");

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::credentials::{InMemoryCredentialStore, format_api_key_hash};

    #[test]
    fn generated_key_has_prefix() {
        let key = generate_plaintext_key();
        assert!(key.starts_with(KEY_PREFIX));
    }

    #[test]
    fn generated_key_is_unique() {
        let a = generate_plaintext_key();
        let b = generate_plaintext_key();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_is_deterministic() {
        let key = "agp_test_key";
        let h1 = hash_key(key);
        let h2 = hash_key(key);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_differs_for_different_keys() {
        let h1 = hash_key("agp_key_one");
        let h2 = hash_key("agp_key_two");
        assert_ne!(h1, h2);
    }

    #[tokio::test]
    async fn initialization_stores_only_a_non_reversible_key_hash() {
        let store = InMemoryCredentialStore::new();
        assert!(ensure_api_key(&store).await.unwrap());
        let stored = store.get("agileplus", keys::API_KEYS).unwrap();
        assert!(stored.starts_with("sha256:"));
        assert!(!stored.starts_with(KEY_PREFIX));
    }

    #[test]
    fn stored_key_hash_validates_without_persisting_plaintext() {
        let store = InMemoryCredentialStore::new();
        let plaintext = "agp_test_secret";
        store
            .set("agileplus", keys::API_KEYS, &format_api_key_hash(plaintext))
            .unwrap();
        assert!(store.validate_api_key(plaintext).unwrap());
        assert!(!store.validate_api_key("agp_wrong_secret").unwrap());
    }
}
