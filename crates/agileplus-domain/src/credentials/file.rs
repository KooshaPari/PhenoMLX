use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::error::CredentialError;
use super::store::CredentialStore;

const ENVELOPE_VERSION: u8 = 1;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;
const KEY_LENGTH: usize = 32;

/// Credential store backed by an AES-256-GCM encrypted JSON envelope.
///
/// The envelope is stored at `~/.agileplus/credentials.enc`. Its encryption
/// key is derived from `AGILEPLUS_CREDENTIAL_KEY` with Argon2id. File-backed
/// credential access is unavailable without that key; there is no plaintext
/// compatibility format or fallback.
pub struct FileCredentialStore {
    path: PathBuf,
    passphrase: Zeroizing<String>,
    cache: RwLock<HashMap<String, HashMap<String, String>>>,
    loaded: RwLock<bool>,
}

#[derive(Deserialize, Serialize)]
struct EncryptedEnvelope {
    version: u8,
    salt: String,
    nonce: String,
    ciphertext: String,
}

impl FileCredentialStore {
    /// Construct a store from the process credential key. This is intentionally
    /// fallible so startup fails closed when file storage has no encryption key.
    pub fn new(path: &Path) -> Result<Self, CredentialError> {
        let passphrase = std::env::var("AGILEPLUS_CREDENTIAL_KEY")
            .map_err(|_| CredentialError::MissingEncryptionKey)?;
        Self::with_passphrase(path, passphrase)
    }

    /// Construct a store with an explicit key, primarily for dependency
    /// injection and deterministic tests. Callers must keep it secret.
    pub fn with_passphrase(path: &Path, passphrase: String) -> Result<Self, CredentialError> {
        if passphrase.is_empty() {
            return Err(CredentialError::MissingEncryptionKey);
        }
        Ok(Self {
            path: path.to_owned(),
            passphrase: Zeroizing::new(passphrase),
            cache: RwLock::new(HashMap::new()),
            loaded: RwLock::new(false),
        })
    }

    fn derive_key(&self, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LENGTH]>, CredentialError> {
        if salt.len() != SALT_LENGTH {
            return Err(CredentialError::Encryption(
                "invalid credential salt length".to_string(),
            ));
        }
        let params = Params::new(19_456, 2, 1, Some(KEY_LENGTH))
            .map_err(|error| CredentialError::Encryption(error.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; KEY_LENGTH]);
        argon2
            .hash_password_into(self.passphrase.as_bytes(), salt, key.as_mut())
            .map_err(|error| CredentialError::Encryption(error.to_string()))?;
        Ok(key)
    }

    fn decrypt(
        &self,
        raw: &[u8],
    ) -> Result<HashMap<String, HashMap<String, String>>, CredentialError> {
        let envelope: EncryptedEnvelope = serde_json::from_slice(raw)
            .map_err(|error| CredentialError::Serialization(error.to_string()))?;
        if envelope.version != ENVELOPE_VERSION {
            return Err(CredentialError::Encryption(format!(
                "unsupported credential envelope version {}",
                envelope.version
            )));
        }
        let salt = decode_field("salt", &envelope.salt, SALT_LENGTH)?;
        let nonce = decode_field("nonce", &envelope.nonce, NONCE_LENGTH)?;
        let ciphertext = STANDARD_NO_PAD
            .decode(envelope.ciphertext)
            .map_err(|error| CredentialError::Encryption(error.to_string()))?;
        let key = self.derive_key(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|error| CredentialError::Encryption(error.to_string()))?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| {
                CredentialError::Encryption("credential file authentication failed".to_string())
            })?;
        serde_json::from_slice(&plaintext)
            .map_err(|error| CredentialError::Serialization(error.to_string()))
    }

    fn encrypt(
        &self,
        credentials: &HashMap<String, HashMap<String, String>>,
    ) -> Result<Vec<u8>, CredentialError> {
        let plaintext = serde_json::to_vec(credentials)
            .map_err(|error| CredentialError::Serialization(error.to_string()))?;
        let mut salt = [0u8; SALT_LENGTH];
        let mut nonce = [0u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce);
        let key = self.derive_key(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|error| CredentialError::Encryption(error.to_string()))?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|_| CredentialError::Encryption("credential encryption failed".to_string()))?;
        let envelope = EncryptedEnvelope {
            version: ENVELOPE_VERSION,
            salt: STANDARD_NO_PAD.encode(salt),
            nonce: STANDARD_NO_PAD.encode(nonce),
            ciphertext: STANDARD_NO_PAD.encode(ciphertext),
        };
        serde_json::to_vec(&envelope)
            .map_err(|error| CredentialError::Serialization(error.to_string()))
    }

    fn ensure_loaded(&self) -> Result<(), CredentialError> {
        if *self.loaded.read().expect("credential loaded lock poisoned") {
            return Ok(());
        }
        let mut loaded = self
            .loaded
            .write()
            .expect("credential loaded lock poisoned");
        if *loaded {
            return Ok(());
        }
        if self.path.exists() {
            let raw = std::fs::read(&self.path)?;
            *self.cache.write().expect("credential cache lock poisoned") = self.decrypt(&raw)?;
        }
        *loaded = true;
        Ok(())
    }

    fn persist_candidate(
        &self,
        credentials: &HashMap<String, HashMap<String, String>>,
    ) -> Result<(), CredentialError> {
        let parent = self.path.parent().ok_or_else(|| {
            CredentialError::Io(std::io::Error::other("credential file path has no parent"))
        })?;
        std::fs::create_dir_all(parent)?;
        let encrypted = self.encrypt(credentials)?;
        let mut suffix = [0u8; 16];
        OsRng.fill_bytes(&mut suffix);
        let temporary = temporary_path(parent, &suffix);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        use std::io::Write as _;
        file.write_all(&encrypted)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        }
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, &self.path)?;
        #[cfg(unix)]
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    }
}

/// Generate a single path component for the atomic-write staging file.
///
/// The random suffix must use the URL-safe alphabet: standard Base64 permits
/// `/`, which would accidentally create a nested path and break atomic writes.
fn temporary_path(parent: &Path, suffix: &[u8; 16]) -> PathBuf {
    parent.join(format!(
        ".credentials-{}.tmp",
        URL_SAFE_NO_PAD.encode(suffix)
    ))
}

fn decode_field(
    field: &str,
    encoded: &str,
    expected_length: usize,
) -> Result<Vec<u8>, CredentialError> {
    let decoded = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|error| CredentialError::Encryption(format!("invalid {field}: {error}")))?;
    if decoded.len() != expected_length {
        return Err(CredentialError::Encryption(format!(
            "invalid {field} length"
        )));
    }
    Ok(decoded)
}

impl CredentialStore for FileCredentialStore {
    fn get(&self, service: &str, key: &str) -> Result<String, CredentialError> {
        self.ensure_loaded()?;
        self.cache
            .read()
            .expect("credential cache lock poisoned")
            .get(service)
            .and_then(|service| service.get(key))
            .cloned()
            .ok_or_else(|| CredentialError::NotFound(key.to_string()))
    }

    fn set(&self, service: &str, key: &str, value: &str) -> Result<(), CredentialError> {
        self.ensure_loaded()?;
        let mut candidate = self
            .cache
            .read()
            .expect("credential cache lock poisoned")
            .clone();
        candidate
            .entry(service.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
        self.persist_candidate(&candidate)?;
        *self.cache.write().expect("credential cache lock poisoned") = candidate;
        Ok(())
    }

    fn delete(&self, service: &str, key: &str) -> Result<(), CredentialError> {
        self.ensure_loaded()?;
        let mut candidate = self
            .cache
            .read()
            .expect("credential cache lock poisoned")
            .clone();
        let removed = candidate
            .get_mut(service)
            .and_then(|service| service.remove(key));
        if removed.is_none() {
            return Err(CredentialError::NotFound(key.to_string()));
        }
        self.persist_candidate(&candidate)?;
        *self.cache.write().expect("credential cache lock poisoned") = candidate;
        Ok(())
    }

    fn list_keys(&self, service: &str) -> Result<Vec<String>, CredentialError> {
        self.ensure_loaded()?;
        Ok(self
            .cache
            .read()
            .expect("credential cache lock poisoned")
            .get(service)
            .map(|service| service.keys().cloned().collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_file_does_not_contain_credential_plaintext() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.enc");
        let store =
            FileCredentialStore::with_passphrase(&path, "test encryption key".to_string()).unwrap();
        store.set("plane", "api-key", "super-secret-value").unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(!raw.contains("super-secret-value"));
        assert_eq!(store.get("plane", "api-key").unwrap(), "super-secret-value");
    }

    #[test]
    fn encrypted_file_reopens_with_same_key_and_rejects_wrong_key() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.enc");
        FileCredentialStore::with_passphrase(&path, "correct key".to_string())
            .unwrap()
            .set("plane", "api-key", "secret")
            .unwrap();
        let reopened =
            FileCredentialStore::with_passphrase(&path, "correct key".to_string()).unwrap();
        assert_eq!(reopened.get("plane", "api-key").unwrap(), "secret");
        let wrong_key =
            FileCredentialStore::with_passphrase(&path, "wrong key".to_string()).unwrap();
        assert!(matches!(
            wrong_key.get("plane", "api-key"),
            Err(CredentialError::Encryption(_))
        ));
    }

    #[test]
    fn missing_file_key_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.enc");
        assert!(matches!(
            FileCredentialStore::with_passphrase(&path, String::new()),
            Err(CredentialError::MissingEncryptionKey)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_persist_leaves_no_temp_file_and_restricts_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.enc");
        let store =
            FileCredentialStore::with_passphrase(&path, "test encryption key".to_string()).unwrap();
        store.set("plane", "api-key", "secret").unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let temp_count = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".credentials-")
            })
            .count();
        assert_eq!(temp_count, 0);
    }

    #[test]
    fn atomic_temp_path_never_contains_a_separator_from_random_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = temporary_path(directory.path(), &[0xff; 16]);
        assert_eq!(path.parent(), Some(directory.path()));
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(".credentials-"));
        assert!(name.ends_with(".tmp"));
        assert!(!name.contains(['/', '\\']));
    }

    #[test]
    fn failed_set_keeps_previous_cached_value() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.enc");
        let store = FileCredentialStore::with_passphrase(&path, "test key".to_string()).unwrap();
        store.set("plane", "api-key", "old").unwrap();
        std::fs::remove_dir_all(directory.path()).unwrap();
        std::fs::write(directory.path(), "not a directory").unwrap();

        assert!(store.set("plane", "api-key", "new").is_err());
        assert_eq!(store.get("plane", "api-key").unwrap(), "old");
    }

    #[test]
    fn failed_delete_keeps_previous_cached_value() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials.enc");
        let store = FileCredentialStore::with_passphrase(&path, "test key".to_string()).unwrap();
        store.set("plane", "api-key", "old").unwrap();
        std::fs::remove_dir_all(directory.path()).unwrap();
        std::fs::write(directory.path(), "not a directory").unwrap();

        assert!(store.delete("plane", "api-key").is_err());
        assert_eq!(store.get("plane", "api-key").unwrap(), "old");
    }
}
