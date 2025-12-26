//! Invite token system for drive sharing
//!
//! Provides secure, signed, time-limited tokens for inviting users to shared drives.
//! Tokens can include permission levels and optional wrapped keys for E2E encryption.

use crate::crypto::access::Permission;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current invite token version
const INVITE_VERSION: u8 = 1;

#[derive(Error, Debug)]
pub enum InviteError {
    #[error("Token expired")]
    Expired,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid token format")]
    InvalidFormat,

    #[error("Token already used")]
    AlreadyUsed,

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// The invite token payload (signed)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvitePayload {
    /// Version for future compatibility
    pub version: u8,
    /// The drive being shared (DriveId hex)
    pub drive_id: String,
    /// Human-readable name of the drive
    pub drive_name: String,
    /// The inviter's NodeId (hex)
    pub inviter: String,
    /// Permission level being granted
    pub permission: Permission,
    /// When the invite was created
    pub created_at: DateTime<Utc>,
    /// When the invite expires
    pub expires_at: DateTime<Utc>,
    /// Optional note/message from inviter
    pub note: Option<String>,
    /// Optional single-use flag
    pub single_use: bool,
    /// Unique token ID for tracking usage
    pub token_id: String,
    /// Optional iroh-docs share ticket for metadata sync
    #[serde(default)]
    pub doc_ticket: Option<String>,
}

impl InvitePayload {
    /// Serialize to bytes for signing
    pub fn to_bytes(&self) -> Result<Vec<u8>, InviteError> {
        json_serialize(self)
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InviteError> {
        json_deserialize(bytes)
    }
}

/// A complete invite token including signature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InviteToken {
    /// The signed payload
    pub payload: InvitePayload,
    /// Ed25519 signature over the payload (hex-encoded)
    pub signature: String,
}

impl InviteToken {
    /// Create a new signed invite token
    pub fn create(
        signing_key: &SigningKey,
        drive_id: &str,
        drive_name: &str,
        permission: Permission,
        validity: Duration,
        note: Option<String>,
        single_use: bool,
        doc_ticket: Option<String>,
    ) -> Result<Self, InviteError> {
        let now = Utc::now();
        let token_id = generate_token_id();

        let payload = InvitePayload {
            version: INVITE_VERSION,
            drive_id: drive_id.to_string(),
            drive_name: drive_name.to_string(),
            inviter: hex::encode(signing_key.verifying_key().to_bytes()),
            permission,
            created_at: now,
            expires_at: now + validity,
            note,
            single_use,
            token_id,
            doc_ticket,
        };

        let payload_bytes = payload.to_bytes()?;
        let signature: Signature = signing_key.sign(&payload_bytes);

        Ok(Self {
            payload,
            signature: hex::encode(signature.to_bytes()),
        })
    }

    /// Verify the token signature
    pub fn verify(&self, verifying_key: &VerifyingKey) -> Result<(), InviteError> {
        let payload_bytes = self.payload.to_bytes()?;
        let sig_bytes = hex::decode(&self.signature).map_err(|_| InviteError::InvalidFormat)?;

        if sig_bytes.len() != 64 {
            return Err(InviteError::InvalidFormat);
        }

        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);

        verifying_key
            .verify(&payload_bytes, &signature)
            .map_err(|_| InviteError::InvalidSignature)
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        self.payload.expires_at < Utc::now()
    }

    /// Check if the token is valid (not expired, signature ok)
    pub fn is_valid(&self, verifying_key: &VerifyingKey) -> bool {
        if self.is_expired() {
            return false;
        }
        self.verify(verifying_key).is_ok()
    }

    /// Serialize to URL-safe base64
    pub fn to_string(&self) -> Result<String, InviteError> {
        let bytes = json_serialize(self)?;
        Ok(base64_url_encode(&bytes))
    }

    /// Parse from URL-safe base64
    pub fn from_string(s: &str) -> Result<Self, InviteError> {
        let bytes = base64_url_decode(s)?;
        json_deserialize(&bytes)
    }

    /// Get the token ID
    pub fn token_id(&self) -> &str {
        &self.payload.token_id
    }
}

/// Builder for creating invite tokens with custom options
pub struct InviteBuilder {
    drive_id: String,
    drive_name: String,
    permission: Permission,
    validity: Duration,
    note: Option<String>,
    single_use: bool,
    doc_ticket: Option<String>,
}

impl InviteBuilder {
    /// Start building an invite for a drive
    pub fn new(drive_id: impl Into<String>, drive_name: impl Into<String>) -> Self {
        Self {
            drive_id: drive_id.into(),
            drive_name: drive_name.into(),
            permission: Permission::Read,
            validity: Duration::days(7), // Default: 1 week
            note: None,
            single_use: false,
            doc_ticket: None,
        }
    }

    /// Set the permission level
    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permission = permission;
        self
    }

    /// Set the validity duration
    pub fn with_validity(mut self, validity: Duration) -> Self {
        self.validity = validity;
        self
    }

    /// Set a note/message
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Make the token single-use
    pub fn single_use(mut self) -> Self {
        self.single_use = true;
        self
    }

    /// Attach a doc share ticket for metadata sync
    pub fn with_doc_ticket(mut self, ticket: impl Into<String>) -> Self {
        self.doc_ticket = Some(ticket.into());
        self
    }

    /// Build and sign the token
    pub fn build(self, signing_key: &SigningKey) -> Result<InviteToken, InviteError> {
        InviteToken::create(
            signing_key,
            &self.drive_id,
            &self.drive_name,
            self.permission,
            self.validity,
            self.note,
            self.single_use,
            self.doc_ticket,
        )
    }
}

/// Tracks used tokens to prevent reuse
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenTracker {
    /// Set of used token IDs
    used_tokens: std::collections::HashSet<String>,
}

impl TokenTracker {
    /// Create a new token tracker
    pub fn new() -> Self {
        Self {
            used_tokens: std::collections::HashSet::new(),
        }
    }

    /// Check if a token has been used
    pub fn is_used(&self, token_id: &str) -> bool {
        self.used_tokens.contains(token_id)
    }

    /// Mark a token as used
    pub fn mark_used(&mut self, token_id: &str) {
        self.used_tokens.insert(token_id.to_string());
    }

    /// Get count of used tokens
    pub fn used_count(&self) -> usize {
        self.used_tokens.len()
    }

    /// Clear old tokens (could be called periodically with expiry info)
    pub fn cleanup(&mut self, valid_ids: &[String]) {
        self.used_tokens.retain(|id| valid_ids.contains(id));
    }
}

/// Generate a unique token ID
fn generate_token_id() -> String {
    let mut bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

/// URL-safe base64 encoding
fn base64_url_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(data)
}

/// URL-safe base64 decoding
fn base64_url_decode(s: &str) -> Result<Vec<u8>, InviteError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| InviteError::InvalidFormat)
}

/// JSON serialization helper
fn json_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, InviteError> {
    serde_json::to_vec(value).map_err(|e| InviteError::SerializationError(e.to_string()))
}

/// JSON deserialization helper
fn json_deserialize<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, InviteError> {
    serde_json::from_slice(bytes).map_err(|e| InviteError::SerializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn generate_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn test_invite_creation() {
        let key = generate_signing_key();
        let token = InviteBuilder::new("drive123", "My Test Drive")
            .with_permission(Permission::Write)
            .with_validity(Duration::hours(24))
            .with_note("Welcome to my drive!")
            .build(&key)
            .unwrap();

        assert_eq!(token.payload.drive_id, "drive123");
        assert_eq!(token.payload.drive_name, "My Test Drive");
        assert_eq!(token.payload.permission, Permission::Write);
        assert!(!token.is_expired());
    }

    #[test]
    fn test_invite_verification() {
        let key = generate_signing_key();
        let token = InviteBuilder::new("drive123", "Test Drive")
            .with_permission(Permission::Read)
            .build(&key)
            .unwrap();

        // Verify with correct key
        assert!(token.verify(&key.verifying_key()).is_ok());

        // Verify with wrong key should fail
        let wrong_key = generate_signing_key();
        assert!(token.verify(&wrong_key.verifying_key()).is_err());
    }

    #[test]
    fn test_invite_serialization() {
        let key = generate_signing_key();
        let token = InviteBuilder::new("drive123", "Serialization Test")
            .with_permission(Permission::Manage)
            .single_use()
            .build(&key)
            .unwrap();

        let token_string = token.to_string().unwrap();
        let restored = InviteToken::from_string(&token_string).unwrap();

        assert_eq!(token.payload.drive_id, restored.payload.drive_id);
        assert_eq!(token.payload.drive_name, restored.payload.drive_name);
        assert_eq!(token.payload.permission, restored.payload.permission);
        assert!(restored.verify(&key.verifying_key()).is_ok());
    }

    #[test]
    fn test_invite_doc_ticket_roundtrip() {
        let key = generate_signing_key();
        let token = InviteBuilder::new("drive123", "Doc Ticket Test")
            .with_permission(Permission::Read)
            .with_doc_ticket("doc-ticket-123")
            .build(&key)
            .unwrap();

        assert_eq!(token.payload.doc_ticket.as_deref(), Some("doc-ticket-123"));

        let token_string = token.to_string().unwrap();
        let restored = InviteToken::from_string(&token_string).unwrap();

        assert_eq!(restored.payload.doc_ticket.as_deref(), Some("doc-ticket-123"));
        assert!(restored.verify(&key.verifying_key()).is_ok());
    }

    #[test]
    fn test_expired_invite() {
        let key = generate_signing_key();
        let token = InviteBuilder::new("drive123", "Expired Drive")
            .with_validity(Duration::seconds(-1)) // Already expired
            .build(&key)
            .unwrap();

        assert!(token.is_expired());
        assert!(!token.is_valid(&key.verifying_key()));
    }

    #[test]
    fn test_token_tracker() {
        let mut tracker = TokenTracker::new();
        let token_id = "token123";

        assert!(!tracker.is_used(token_id));

        tracker.mark_used(token_id);
        assert!(tracker.is_used(token_id));

        // Second use should still show as used
        assert!(tracker.is_used(token_id));
    }

    #[test]
    fn test_invite_builder_defaults() {
        let key = generate_signing_key();
        let token = InviteBuilder::new("drive123", "Default Test").build(&key).unwrap();

        // Default permission is Read
        assert_eq!(token.payload.permission, Permission::Read);
        // Default single_use is false
        assert!(!token.payload.single_use);
    }
}
