//! Drive encryption management
//!
//! Provides a centralized manager for drive encryption keys and operations.
//! Keys are stored encrypted (wrapped) per user using their X25519 public key.

use crate::crypto::{
    DriveEncryption, DriveKey, EncryptionError, KeyExchangeError, KeyExchangePair, WrappedKey,
};
use crate::storage::Database;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use x25519_dalek::PublicKey;

/// Manages encryption keys for all drives
///
/// Handles:
/// - Generating drive keys for new drives
/// - Storing wrapped keys per user
/// - Unwrapping keys for authorized users
/// - Encrypting/decrypting file content
pub struct EncryptionManager {
    /// Our key exchange keypair for unwrapping drive keys
    exchange_keypair: KeyExchangePair,
    /// Cached unwrapped drive keys (drive_id_hex -> DriveKey)
    cached_keys: RwLock<HashMap<String, DriveKey>>,
    /// Database for persistent storage
    db: Arc<Database>,
}

impl EncryptionManager {
    /// Create a new EncryptionManager
    ///
    /// Loads or generates a key exchange keypair for this node.
    pub fn new(db: Arc<Database>) -> Result<Self, EncryptionManagerError> {
        // Try to load existing key exchange keypair
        let exchange_keypair = match db.get_key_exchange_keypair() {
            Ok(Some(bytes)) => {
                tracing::info!("Loaded existing key exchange keypair");
                KeyExchangePair::from_bytes(&bytes)
            }
            Ok(None) => {
                // Generate new keypair
                let keypair = KeyExchangePair::generate();
                let bytes = keypair.secret_bytes();
                db.save_key_exchange_keypair(&bytes)
                    .map_err(|e| EncryptionManagerError::StorageError(e.to_string()))?;
                tracing::info!("Generated new key exchange keypair");
                keypair
            }
            Err(e) => {
                return Err(EncryptionManagerError::StorageError(e.to_string()));
            }
        };

        Ok(Self {
            exchange_keypair,
            cached_keys: RwLock::new(HashMap::new()),
            db,
        })
    }

    /// Get our public key for receiving wrapped drive keys
    pub fn public_key(&self) -> [u8; 32] {
        self.exchange_keypair.public_bytes()
    }

    /// Generate a new drive key and wrap it for the owner
    ///
    /// Returns the wrapped key that should be stored with the drive.
    pub fn generate_drive_key(
        &self,
        drive_id: &str,
        owner_public_key: &[u8; 32],
    ) -> Result<WrappedKey, EncryptionManagerError> {
        let drive_key = DriveKey::generate();

        // Wrap for owner
        let owner_pk = PublicKey::from(*owner_public_key);
        let wrapped = KeyExchangePair::wrap_key_for(&owner_pk, drive_key.as_bytes())
            .map_err(|e| EncryptionManagerError::KeyExchangeError(e))?;

        // Cache the unwrapped key for immediate use
        {
            let mut cache = self.cached_keys.blocking_write();
            cache.insert(drive_id.to_string(), drive_key);
        }

        // Store wrapped key in database
        let wrapped_bytes = wrapped.to_bytes();
        self.db
            .save_drive_key(drive_id, &wrapped_bytes)
            .map_err(|e| EncryptionManagerError::StorageError(e.to_string()))?;

        Ok(wrapped)
    }

    /// Import a wrapped drive key from an invite
    ///
    /// Unwraps the key using our private key and caches it.
    pub fn import_drive_key(
        &self,
        drive_id: &str,
        wrapped: &WrappedKey,
    ) -> Result<(), EncryptionManagerError> {
        // Unwrap the key
        let drive_key_bytes = self
            .exchange_keypair
            .unwrap_key(wrapped)
            .map_err(|e| EncryptionManagerError::KeyExchangeError(e))?;

        let drive_key = DriveKey::from_bytes(drive_key_bytes);

        // Cache the key
        {
            let mut cache = self.cached_keys.blocking_write();
            cache.insert(drive_id.to_string(), drive_key);
        }

        // Store wrapped key in database for persistence
        let wrapped_bytes = wrapped.to_bytes();
        self.db
            .save_drive_key(drive_id, &wrapped_bytes)
            .map_err(|e| EncryptionManagerError::StorageError(e.to_string()))?;

        Ok(())
    }

    /// Get the encryption handler for a drive
    ///
    /// Returns None if we don't have access to the drive's key.
    pub async fn get_encryption(&self, drive_id: &str) -> Option<DriveEncryption> {
        // First check cache
        {
            let cache = self.cached_keys.read().await;
            if let Some(key) = cache.get(drive_id) {
                return Some(DriveEncryption::new(key.clone()));
            }
        }

        // Try to load from database
        if let Ok(Some(wrapped_bytes)) = self.db.get_drive_key(drive_id) {
            if let Ok(wrapped) = WrappedKey::from_bytes(&wrapped_bytes) {
                if let Ok(key_bytes) = self.exchange_keypair.unwrap_key(&wrapped) {
                    let drive_key = DriveKey::from_bytes(key_bytes);

                    // Cache for future use
                    {
                        let mut cache = self.cached_keys.write().await;
                        cache.insert(drive_id.to_string(), drive_key.clone());
                    }

                    return Some(DriveEncryption::new(drive_key));
                }
            }
        }

        None
    }

    /// Wrap a drive key for a new user
    ///
    /// Called when granting access to a drive.
    pub fn wrap_key_for_user(
        &self,
        drive_id: &str,
        user_public_key: &[u8; 32],
    ) -> Result<WrappedKey, EncryptionManagerError> {
        // Get the drive key from cache or database
        let drive_key = {
            let cache = self.cached_keys.blocking_read();
            cache.get(drive_id).cloned()
        };

        let drive_key = match drive_key {
            Some(key) => key,
            None => {
                // Try to load from database
                let wrapped_bytes = self
                    .db
                    .get_drive_key(drive_id)
                    .map_err(|e| EncryptionManagerError::StorageError(e.to_string()))?
                    .ok_or_else(|| EncryptionManagerError::KeyNotFound(drive_id.to_string()))?;

                let wrapped = WrappedKey::from_bytes(&wrapped_bytes)
                    .map_err(|e| EncryptionManagerError::KeyExchangeError(e))?;

                let key_bytes = self
                    .exchange_keypair
                    .unwrap_key(&wrapped)
                    .map_err(|e| EncryptionManagerError::KeyExchangeError(e))?;

                DriveKey::from_bytes(key_bytes)
            }
        };

        // Wrap for new user
        let user_pk = PublicKey::from(*user_public_key);
        KeyExchangePair::wrap_key_for(&user_pk, drive_key.as_bytes())
            .map_err(|e| EncryptionManagerError::KeyExchangeError(e))
    }

    /// Encrypt file content for a drive
    pub async fn encrypt_file(
        &self,
        drive_id: &str,
        path: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, EncryptionManagerError> {
        let encryption = self
            .get_encryption(drive_id)
            .await
            .ok_or_else(|| EncryptionManagerError::KeyNotFound(drive_id.to_string()))?;

        encryption
            .encrypt(plaintext, path)
            .map_err(|e| EncryptionManagerError::EncryptionError(e))
    }

    /// Decrypt file content from a drive
    pub async fn decrypt_file(
        &self,
        drive_id: &str,
        path: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, EncryptionManagerError> {
        let encryption = self
            .get_encryption(drive_id)
            .await
            .ok_or_else(|| EncryptionManagerError::KeyNotFound(drive_id.to_string()))?;

        encryption
            .decrypt(ciphertext, path)
            .map_err(|e| EncryptionManagerError::EncryptionError(e))
    }

    /// Check if we have the key for a drive
    pub async fn has_key(&self, drive_id: &str) -> bool {
        self.get_encryption(drive_id).await.is_some()
    }

    /// Clear cached keys (for security, e.g., on app lock)
    ///
    /// Returns true if any keys were actually cleared.
    #[allow(dead_code)]
    pub async fn clear_cache(&self) -> bool {
        let mut cache = self.cached_keys.write().await;
        if cache.is_empty() {
            return false;
        }
        let count = cache.len();
        cache.clear();
        tracing::info!("Encryption key cache cleared ({} keys)", count);
        true
    }
}

/// Errors from the encryption manager
#[derive(Debug)]
pub enum EncryptionManagerError {
    /// Key not found for drive
    KeyNotFound(String),
    /// Key exchange error
    KeyExchangeError(KeyExchangeError),
    /// Encryption/decryption error
    EncryptionError(EncryptionError),
    /// Storage error
    StorageError(String),
}

impl std::fmt::Display for EncryptionManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncryptionManagerError::KeyNotFound(id) => {
                write!(f, "No encryption key for drive: {}", id)
            }
            EncryptionManagerError::KeyExchangeError(e) => write!(f, "Key exchange error: {}", e),
            EncryptionManagerError::EncryptionError(e) => write!(f, "Encryption error: {}", e),
            EncryptionManagerError::StorageError(e) => write!(f, "Storage error: {}", e),
        }
    }
}

impl std::error::Error for EncryptionManagerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_encrypt_decrypt_file() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Database::open(&dir.path().join("test.redb")).unwrap());

        let manager = EncryptionManager::new(db).unwrap();

        // Generate a key for a drive
        let owner_pk = manager.public_key();
        let _wrapped = manager.generate_drive_key("test-drive", &owner_pk).unwrap();

        // Encrypt some content
        let plaintext = b"Hello, encrypted world!";
        let ciphertext = manager
            .encrypt_file("test-drive", "test.txt", plaintext)
            .await
            .unwrap();

        // Decrypt
        let decrypted = manager
            .decrypt_file("test-drive", "test.txt", &ciphertext)
            .await
            .unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }
}
