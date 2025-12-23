//! Drive encryption using ChaCha20-Poly1305
//!
//! Provides E2E encryption for files in shared drives.
//! Each drive has a unique 256-bit master key that is used to derive
//! per-file encryption keys using BLAKE3 key derivation.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use thiserror::Error;

/// Size of ChaCha20 nonce (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Size of Poly1305 authentication tag (128 bits = 16 bytes)
const TAG_SIZE: usize = 16;

/// Chunk size for streaming encryption (64KB - optimal for performance)
const CHUNK_SIZE: usize = 64 * 1024;

/// Current encryption version for forward compatibility
const ENCRYPTION_VERSION: u8 = 1;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid ciphertext format")]
    InvalidFormat,

    #[error("Invalid key length")]
    InvalidKeyLength,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unsupported encryption version: {0}")]
    UnsupportedVersion(u8),
}

/// Master key for a shared drive (256 bits)
#[derive(Clone, Serialize, Deserialize)]
pub struct DriveKey {
    bytes: [u8; 32],
}

impl DriveKey {
    /// Generate a new random drive key
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    /// Get raw key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Derive a file-specific key using BLAKE3 KDF
    ///
    /// This ensures each file uses a unique key derived from the master key,
    /// preventing key reuse attacks.
    pub fn derive_file_key(&self, file_path: &str) -> [u8; 32] {
        let context = format!("gix-drive:file-key:{}", file_path);
        blake3::derive_key(&context, &self.bytes)
    }

    pub fn derive_metadata_key(&self) -> [u8; 32] {
        blake3::derive_key("gix-drive:metadata-key", &self.bytes)
    }
}

impl std::fmt::Debug for DriveKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never expose key bytes in debug output
        f.debug_struct("DriveKey")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl Drop for DriveKey {
    fn drop(&mut self) {
        // Zeroize key on drop to minimize exposure time
        self.bytes.fill(0);
    }
}

/// Handles encryption and decryption for drive files
pub struct DriveEncryption {
    key: DriveKey,
}

impl DriveEncryption {
    /// Create a new DriveEncryption with an existing key
    pub fn new(key: DriveKey) -> Self {
        Self { key }
    }

    /// Create with a newly generated key
    pub fn generate() -> Self {
        Self {
            key: DriveKey::generate(),
        }
    }

    /// Get the drive's master key (for key wrapping/sharing)
    pub fn key(&self) -> &DriveKey {
        &self.key
    }

    /// Encrypt a small payload (metadata, small files)
    ///
    /// Format: [version:1][nonce:12][ciphertext+tag:N+16]
    pub fn encrypt(&self, plaintext: &[u8], context: &str) -> Result<Vec<u8>, EncryptionError> {
        let key_bytes = self.key.derive_file_key(context);
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| EncryptionError::InvalidKeyLength)?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // Build output: version + nonce + ciphertext
        let mut output = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
        output.push(ENCRYPTION_VERSION);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// Decrypt a small payload
    pub fn decrypt(&self, ciphertext: &[u8], context: &str) -> Result<Vec<u8>, EncryptionError> {
        // Check minimum size: version + nonce + tag
        if ciphertext.len() < 1 + NONCE_SIZE + TAG_SIZE {
            return Err(EncryptionError::InvalidFormat);
        }

        // Check version
        let version = ciphertext[0];
        if version != ENCRYPTION_VERSION {
            return Err(EncryptionError::UnsupportedVersion(version));
        }

        let key_bytes = self.key.derive_file_key(context);
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| EncryptionError::InvalidKeyLength)?;

        let nonce = Nonce::from_slice(&ciphertext[1..1 + NONCE_SIZE]);
        let encrypted_data = &ciphertext[1 + NONCE_SIZE..];

        let plaintext = cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        Ok(plaintext)
    }

    /// Encrypt a file using streaming (for large files)
    ///
    /// Each chunk is encrypted independently with a unique nonce derived from
    /// the chunk index. This allows parallel encryption/decryption.
    ///
    /// Format: [version:1][chunk_count:8][chunks...]
    /// Each chunk: [nonce:12][ciphertext+tag:N+16]
    pub fn encrypt_stream<R: Read, W: Write>(
        &self,
        mut reader: R,
        mut writer: W,
        file_path: &str,
    ) -> Result<(), EncryptionError> {
        let key_bytes = self.key.derive_file_key(file_path);
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| EncryptionError::InvalidKeyLength)?;

        // Write version
        writer.write_all(&[ENCRYPTION_VERSION])?;

        // We'll write chunk count at the end, so write placeholder
        let chunk_count_pos = writer.write(&[0u8; 8])?;
        let _ = chunk_count_pos; // Acknowledge we wrote it

        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut chunk_index: u64 = 0;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            // Derive unique nonce from chunk index
            let mut nonce_bytes = [0u8; NONCE_SIZE];
            // Use BLAKE3 to derive nonce from file path + chunk index
            let nonce_material = blake3::derive_key(
                &format!("gix-drive:nonce:{}:{}", file_path, chunk_index),
                &key_bytes,
            );
            nonce_bytes.copy_from_slice(&nonce_material[..NONCE_SIZE]);
            let nonce = Nonce::from_slice(&nonce_bytes);

            // Encrypt chunk
            let ciphertext = cipher
                .encrypt(nonce, &buffer[..bytes_read])
                .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

            // Write chunk: nonce + ciphertext
            writer.write_all(&nonce_bytes)?;
            writer.write_all(&ciphertext)?;

            chunk_index += 1;
        }

        // Note: In a real implementation, we'd seek back and write chunk count
        // For now, we rely on detecting EOF during decryption

        Ok(())
    }

    /// Decrypt a file using streaming
    pub fn decrypt_stream<R: Read, W: Write>(
        &self,
        mut reader: R,
        mut writer: W,
        file_path: &str,
    ) -> Result<(), EncryptionError> {
        let key_bytes = self.key.derive_file_key(file_path);
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| EncryptionError::InvalidKeyLength)?;

        // Read version
        let mut version = [0u8; 1];
        reader.read_exact(&mut version)?;
        if version[0] != ENCRYPTION_VERSION {
            return Err(EncryptionError::UnsupportedVersion(version[0]));
        }

        // Skip chunk count placeholder
        let mut _chunk_count = [0u8; 8];
        reader.read_exact(&mut _chunk_count)?;

        let mut chunk_index: u64 = 0;
        let max_chunk_ciphertext_size = CHUNK_SIZE + TAG_SIZE;

        loop {
            // Read nonce
            let mut nonce_bytes = [0u8; NONCE_SIZE];
            match reader.read_exact(&mut nonce_bytes) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }

            // Read ciphertext (up to max chunk size + tag)
            // We need to read the actual ciphertext size, which varies
            let mut ciphertext_buf = vec![0u8; max_chunk_ciphertext_size];
            let bytes_read = reader.read(&mut ciphertext_buf)?;
            if bytes_read == 0 {
                break;
            }

            // Derive expected nonce for verification
            let mut expected_nonce = [0u8; NONCE_SIZE];
            let nonce_material = blake3::derive_key(
                &format!("gix-drive:nonce:{}:{}", file_path, chunk_index),
                &key_bytes,
            );
            expected_nonce.copy_from_slice(&nonce_material[..NONCE_SIZE]);

            // Verify nonce matches (prevents chunk reordering attacks)
            if nonce_bytes != expected_nonce {
                return Err(EncryptionError::DecryptionFailed(
                    "Nonce mismatch - possible data corruption or tampering".into(),
                ));
            }

            let nonce = Nonce::from_slice(&nonce_bytes);

            // Decrypt chunk
            let plaintext = cipher
                .decrypt(nonce, &ciphertext_buf[..bytes_read])
                .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

            writer.write_all(&plaintext)?;
            chunk_index += 1;
        }

        Ok(())
    }

    /// Encrypt a file path/name (for metadata privacy)
    pub fn encrypt_path(&self, path: &str) -> Result<String, EncryptionError> {
        let key_bytes = self.key.derive_metadata_key();
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| EncryptionError::InvalidKeyLength)?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, path.as_bytes())
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // Encode as: nonce + ciphertext in hex for safe storage
        let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(hex::encode(combined))
    }

    /// Decrypt a file path/name
    pub fn decrypt_path(&self, encrypted_hex: &str) -> Result<String, EncryptionError> {
        let combined = hex::decode(encrypted_hex).map_err(|_| EncryptionError::InvalidFormat)?;

        if combined.len() < NONCE_SIZE + TAG_SIZE {
            return Err(EncryptionError::InvalidFormat);
        }

        let key_bytes = self.key.derive_metadata_key();
        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| EncryptionError::InvalidKeyLength)?;

        let nonce = Nonce::from_slice(&combined[..NONCE_SIZE]);
        let ciphertext = &combined[NONCE_SIZE..];

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|_| EncryptionError::DecryptionFailed("Invalid UTF-8 in path".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drive_key_generation() {
        let key1 = DriveKey::generate();
        let key2 = DriveKey::generate();

        // Keys should be different
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_file_key_derivation() {
        let key = DriveKey::generate();
        let file_key1 = key.derive_file_key("test.txt");
        let file_key2 = key.derive_file_key("test.txt");
        let file_key3 = key.derive_file_key("other.txt");

        // Same file should derive same key
        assert_eq!(file_key1, file_key2);
        // Different files should derive different keys
        assert_ne!(file_key1, file_key3);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let encryption = DriveEncryption::generate();
        let plaintext = b"Hello, encrypted world!";

        let ciphertext = encryption.encrypt(plaintext, "test.txt").unwrap();
        let decrypted = encryption.decrypt(&ciphertext, "test.txt").unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_wrong_context_fails() {
        let encryption = DriveEncryption::generate();
        let plaintext = b"Secret data";

        let ciphertext = encryption.encrypt(plaintext, "file1.txt").unwrap();
        let result = encryption.decrypt(&ciphertext, "file2.txt");

        assert!(result.is_err());
    }

    #[test]
    fn test_stream_encrypt_decrypt() {
        let encryption = DriveEncryption::generate();
        let plaintext = b"This is some test data for streaming encryption";

        let mut encrypted = Vec::new();
        encryption
            .encrypt_stream(plaintext.as_slice(), &mut encrypted, "stream.txt")
            .unwrap();

        let mut decrypted = Vec::new();
        encryption
            .decrypt_stream(encrypted.as_slice(), &mut decrypted, "stream.txt")
            .unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_path_encryption() {
        let encryption = DriveEncryption::generate();
        let path = "documents/secret/passwords.txt";

        let encrypted = encryption.encrypt_path(path).unwrap();
        let decrypted = encryption.decrypt_path(&encrypted).unwrap();

        assert_eq!(path, decrypted);
        // Encrypted should be different each time (random nonce)
        let encrypted2 = encryption.encrypt_path(path).unwrap();
        assert_ne!(encrypted, encrypted2);
    }

    #[test]
    fn test_large_file_streaming() {
        let encryption = DriveEncryption::generate();
        // 256KB of data (4 chunks)
        let plaintext: Vec<u8> = (0..256 * 1024).map(|i| (i % 256) as u8).collect();

        let mut encrypted = Vec::new();
        encryption
            .encrypt_stream(plaintext.as_slice(), &mut encrypted, "large.bin")
            .unwrap();

        let mut decrypted = Vec::new();
        encryption
            .decrypt_stream(encrypted.as_slice(), &mut decrypted, "large.bin")
            .unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
