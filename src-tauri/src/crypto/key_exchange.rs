//! Key exchange and wrapping using X25519
//!
//! Provides secure key exchange between peers for sharing drive encryption keys.
//! Uses X25519 Diffie-Hellman for key agreement and ChaCha20-Poly1305 for key wrapping.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret, StaticSecret};

/// Size of ChaCha20 nonce
const NONCE_SIZE: usize = 12;

/// Size of X25519 public key
const PUBLIC_KEY_SIZE: usize = 32;

#[derive(Error, Debug)]
pub enum KeyExchangeError {
    #[error("Failed to generate keypair")]
    GenerationFailed,

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Key wrapping failed: {0}")]
    WrapFailed(String),

    #[error("Key unwrapping failed: {0}")]
    UnwrapFailed(String),

    #[error("Invalid wrapped key format")]
    InvalidFormat,
}

/// X25519 keypair for key exchange
///
/// This is the user's long-term keypair used to receive wrapped drive keys.
pub struct KeyExchangePair {
    secret: StaticSecret,
    public: PublicKey,
}

impl KeyExchangePair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::thread_rng());
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Create from existing secret key bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let secret = StaticSecret::from(*bytes);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Get the secret key bytes for secure storage
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.secret.to_bytes()
    }

    /// Get the public key for sharing
    pub fn public_key(&self) -> &PublicKey {
        &self.public
    }

    /// Get the public key bytes
    pub fn public_bytes(&self) -> [u8; 32] {
        self.public.to_bytes()
    }

    /// Perform Diffie-Hellman with a peer's public key to derive shared secret
    pub fn diffie_hellman(&self, peer_public: &PublicKey) -> SharedSecret {
        self.secret.diffie_hellman(peer_public)
    }

    /// Wrap (encrypt) a drive key for a specific recipient
    ///
    /// Uses ephemeral ECDH: generates a one-time keypair, performs DH with
    /// recipient's public key, and encrypts the drive key with the shared secret.
    ///
    /// Format: [ephemeral_public:32][nonce:12][encrypted_key+tag:32+16]
    pub fn wrap_key_for(
        recipient_public: &PublicKey,
        drive_key: &[u8; 32],
    ) -> Result<WrappedKey, KeyExchangeError> {
        // Generate ephemeral keypair for this key wrap
        let ephemeral_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
        let ephemeral_public = PublicKey::from(&ephemeral_secret);

        // Derive shared secret
        let shared_secret = ephemeral_secret.diffie_hellman(recipient_public);

        // Derive encryption key from shared secret using BLAKE3
        let wrap_key = blake3::derive_key("gix-drive:key-wrap", shared_secret.as_bytes());

        // Encrypt the drive key
        let cipher = ChaCha20Poly1305::new_from_slice(&wrap_key)
            .map_err(|e| KeyExchangeError::WrapFailed(e.to_string()))?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted = cipher
            .encrypt(nonce, drive_key.as_slice())
            .map_err(|e| KeyExchangeError::WrapFailed(e.to_string()))?;

        Ok(WrappedKey {
            ephemeral_public: ephemeral_public.to_bytes(),
            nonce: nonce_bytes,
            ciphertext: encrypted,
        })
    }

    /// Unwrap (decrypt) a drive key sent to us
    pub fn unwrap_key(&self, wrapped: &WrappedKey) -> Result<[u8; 32], KeyExchangeError> {
        // Reconstruct ephemeral public key
        let ephemeral_public = PublicKey::from(wrapped.ephemeral_public);

        // Derive shared secret
        let shared_secret = self.secret.diffie_hellman(&ephemeral_public);

        // Derive decryption key
        let wrap_key = blake3::derive_key("gix-drive:key-wrap", shared_secret.as_bytes());

        // Decrypt the drive key
        let cipher = ChaCha20Poly1305::new_from_slice(&wrap_key)
            .map_err(|e| KeyExchangeError::UnwrapFailed(e.to_string()))?;

        let nonce = Nonce::from_slice(&wrapped.nonce);

        let decrypted = cipher
            .decrypt(nonce, wrapped.ciphertext.as_slice())
            .map_err(|e| KeyExchangeError::UnwrapFailed(e.to_string()))?;

        if decrypted.len() != 32 {
            return Err(KeyExchangeError::InvalidFormat);
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&decrypted);
        Ok(key)
    }
}

impl Clone for KeyExchangePair {
    fn clone(&self) -> Self {
        Self::from_bytes(&self.secret_bytes())
    }
}

impl std::fmt::Debug for KeyExchangePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyExchangePair")
            .field("secret", &"[REDACTED]")
            .field("public", &hex::encode(self.public.as_bytes()))
            .finish()
    }
}

/// A drive key wrapped for a specific recipient
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WrappedKey {
    /// Ephemeral public key used for this wrap operation
    pub ephemeral_public: [u8; PUBLIC_KEY_SIZE],
    /// Random nonce for encryption
    pub nonce: [u8; NONCE_SIZE],
    /// Encrypted drive key + auth tag
    pub ciphertext: Vec<u8>,
}

impl WrappedKey {
    /// Serialize to bytes for storage/transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(PUBLIC_KEY_SIZE + NONCE_SIZE + self.ciphertext.len());
        bytes.extend_from_slice(&self.ephemeral_public);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeyExchangeError> {
        // Minimum size: ephemeral_public + nonce + encrypted_key + tag
        let min_size = PUBLIC_KEY_SIZE + NONCE_SIZE + 32 + 16;
        if bytes.len() < min_size {
            return Err(KeyExchangeError::InvalidFormat);
        }

        let mut ephemeral_public = [0u8; PUBLIC_KEY_SIZE];
        ephemeral_public.copy_from_slice(&bytes[..PUBLIC_KEY_SIZE]);

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&bytes[PUBLIC_KEY_SIZE..PUBLIC_KEY_SIZE + NONCE_SIZE]);

        let ciphertext = bytes[PUBLIC_KEY_SIZE + NONCE_SIZE..].to_vec();

        Ok(Self {
            ephemeral_public,
            nonce,
            ciphertext,
        })
    }
}

/// Manages wrapped keys for all users in a drive
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KeyRing {
    /// Map of user NodeId (hex) to their wrapped key
    wrapped_keys: std::collections::HashMap<String, WrappedKey>,
}

impl KeyRing {
    /// Create an empty keyring
    pub fn new() -> Self {
        Self {
            wrapped_keys: std::collections::HashMap::new(),
        }
    }

    /// Add a wrapped key for a user
    pub fn add(&mut self, node_id: &str, wrapped_key: WrappedKey) {
        self.wrapped_keys.insert(node_id.to_string(), wrapped_key);
    }

    /// Get a user's wrapped key
    pub fn get(&self, node_id: &str) -> Option<&WrappedKey> {
        self.wrapped_keys.get(node_id)
    }

    /// Remove a user's wrapped key (revocation)
    pub fn remove(&mut self, node_id: &str) -> Option<WrappedKey> {
        self.wrapped_keys.remove(node_id)
    }

    /// Check if a user has access
    pub fn has_user(&self, node_id: &str) -> bool {
        self.wrapped_keys.contains_key(node_id)
    }

    /// Get all user IDs with access
    pub fn users(&self) -> Vec<String> {
        self.wrapped_keys.keys().cloned().collect()
    }

    /// Get the number of users with access
    pub fn len(&self) -> usize {
        self.wrapped_keys.len()
    }

    /// Check if the keyring is empty
    pub fn is_empty(&self) -> bool {
        self.wrapped_keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let pair1 = KeyExchangePair::generate();
        let pair2 = KeyExchangePair::generate();

        // Public keys should be different
        assert_ne!(pair1.public_bytes(), pair2.public_bytes());
    }

    #[test]
    fn test_keypair_serialization() {
        let pair = KeyExchangePair::generate();
        let secret_bytes = pair.secret_bytes();
        let restored = KeyExchangePair::from_bytes(&secret_bytes);

        assert_eq!(pair.public_bytes(), restored.public_bytes());
    }

    #[test]
    fn test_diffie_hellman() {
        let alice = KeyExchangePair::generate();
        let bob = KeyExchangePair::generate();

        let alice_shared = alice.diffie_hellman(bob.public_key());
        let bob_shared = bob.diffie_hellman(alice.public_key());

        assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
    }

    #[test]
    fn test_key_wrap_unwrap() {
        let alice = KeyExchangePair::generate();
        let bob = KeyExchangePair::generate();

        let drive_key: [u8; 32] = rand::random();

        // Alice wraps drive key for Bob
        let wrapped = KeyExchangePair::wrap_key_for(bob.public_key(), &drive_key).unwrap();

        // Bob unwraps the drive key
        let unwrapped = bob.unwrap_key(&wrapped).unwrap();

        assert_eq!(drive_key, unwrapped);
    }

    #[test]
    fn test_wrong_recipient_cannot_unwrap() {
        let alice = KeyExchangePair::generate();
        let bob = KeyExchangePair::generate();
        let eve = KeyExchangePair::generate();

        let drive_key: [u8; 32] = rand::random();

        // Alice wraps drive key for Bob
        let wrapped = KeyExchangePair::wrap_key_for(bob.public_key(), &drive_key).unwrap();

        // Eve tries to unwrap - should fail
        let result = eve.unwrap_key(&wrapped);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrapped_key_serialization() {
        let bob = KeyExchangePair::generate();
        let drive_key: [u8; 32] = rand::random();

        let wrapped = KeyExchangePair::wrap_key_for(bob.public_key(), &drive_key).unwrap();
        let bytes = wrapped.to_bytes();
        let restored = WrappedKey::from_bytes(&bytes).unwrap();

        let unwrapped = bob.unwrap_key(&restored).unwrap();
        assert_eq!(drive_key, unwrapped);
    }

    #[test]
    fn test_keyring_operations() {
        let mut keyring = KeyRing::new();
        let bob = KeyExchangePair::generate();
        let drive_key: [u8; 32] = rand::random();

        let wrapped = KeyExchangePair::wrap_key_for(bob.public_key(), &drive_key).unwrap();

        keyring.add("bob123", wrapped);

        assert!(keyring.has_user("bob123"));
        assert!(!keyring.has_user("alice456"));
        assert_eq!(keyring.len(), 1);

        let retrieved = keyring.get("bob123").unwrap();
        let unwrapped = bob.unwrap_key(retrieved).unwrap();
        assert_eq!(drive_key, unwrapped);

        keyring.remove("bob123");
        assert!(!keyring.has_user("bob123"));
    }
}
