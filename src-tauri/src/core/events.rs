//! Drive events for real-time synchronization
//!
//! Events are broadcast via gossip protocol and used for:
//! - Notifying peers of file changes
//! - Tracking user presence (join/leave)
//! - Sync progress reporting
//!
//! All gossip messages are signed for authentication.

use crate::crypto::{Identity, NodeId};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Events broadcast over gossip for real-time updates
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DriveEvent {
    /// A file was created or modified
    FileChanged {
        path: PathBuf,
        /// BLAKE3 hash of file content (hex string)
        hash: String,
        size: u64,
        modified_by: NodeId,
        timestamp: DateTime<Utc>,
    },

    /// A file was deleted
    FileDeleted {
        path: PathBuf,
        deleted_by: NodeId,
        timestamp: DateTime<Utc>,
    },

    /// A file is being edited (advisory lock)
    FileEditStarted {
        path: PathBuf,
        editor: NodeId,
    },

    /// A file edit session ended
    FileEditEnded {
        path: PathBuf,
        editor: NodeId,
    },

    /// A file lock was acquired
    FileLockAcquired {
        path: PathBuf,
        holder: NodeId,
        lock_type: String,
        expires_at: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },

    /// A file lock was released
    FileLockReleased {
        path: PathBuf,
        holder: NodeId,
        timestamp: DateTime<Utc>,
    },

    /// User joined the drive
    UserJoined {
        user: NodeId,
        timestamp: DateTime<Utc>,
    },

    /// User left the drive
    UserLeft {
        user: NodeId,
        timestamp: DateTime<Utc>,
    },

    /// Sync progress update (Phase 2b)
    SyncProgress {
        path: PathBuf,
        bytes_transferred: u64,
        total_bytes: u64,
    },

    /// Sync completed for a file (Phase 2b)
    SyncComplete {
        path: PathBuf,
        hash: String,
    },
}

impl DriveEvent {
    /// Get the event type as a string for frontend categorization
    pub fn event_type(&self) -> &'static str {
        match self {
            DriveEvent::FileChanged { .. } => "FileChanged",
            DriveEvent::FileDeleted { .. } => "FileDeleted",
            DriveEvent::FileEditStarted { .. } => "FileEditStarted",
            DriveEvent::FileEditEnded { .. } => "FileEditEnded",
            DriveEvent::FileLockAcquired { .. } => "FileLockAcquired",
            DriveEvent::FileLockReleased { .. } => "FileLockReleased",
            DriveEvent::UserJoined { .. } => "UserJoined",
            DriveEvent::UserLeft { .. } => "UserLeft",
            DriveEvent::SyncProgress { .. } => "SyncProgress",
            DriveEvent::SyncComplete { .. } => "SyncComplete",
        }
    }

    /// Get timestamp if the event has one
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            DriveEvent::FileChanged { timestamp, .. } => Some(*timestamp),
            DriveEvent::FileDeleted { timestamp, .. } => Some(*timestamp),
            DriveEvent::FileLockAcquired { timestamp, .. } => Some(*timestamp),
            DriveEvent::FileLockReleased { timestamp, .. } => Some(*timestamp),
            DriveEvent::UserJoined { timestamp, .. } => Some(*timestamp),
            DriveEvent::UserLeft { timestamp, .. } => Some(*timestamp),
            _ => None,
        }
    }
}

/// DTO for sending drive events to frontend via Tauri emit
#[derive(Clone, Debug, Serialize)]
pub struct DriveEventDto {
    /// Drive this event belongs to (hex string)
    pub drive_id: String,
    /// Event type for frontend routing
    pub event_type: String,
    /// Full event payload as JSON
    pub payload: serde_json::Value,
    /// ISO 8601 timestamp
    pub timestamp: String,
}

impl DriveEventDto {
    /// Create DTO from drive ID and event
    pub fn from_event(drive_id: &str, event: &DriveEvent) -> Self {
        let timestamp = event
            .timestamp()
            .unwrap_or_else(Utc::now)
            .to_rfc3339();

        Self {
            drive_id: drive_id.to_string(),
            event_type: event.event_type().to_string(),
            payload: serde_json::to_value(event).unwrap_or(serde_json::Value::Null),
            timestamp,
        }
    }
}

/// Signed envelope for gossip messages
///
/// Wraps a DriveEvent with the sender's identity and a cryptographic signature
/// to authenticate the message and prevent forgery/replay attacks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedGossipMessage {
    /// The actual event payload
    pub event: DriveEvent,
    /// Sender's public key (NodeId)
    pub sender: NodeId,
    /// Unix timestamp (milliseconds) when message was created
    pub timestamp_ms: i64,
    /// Ed25519 signature over (event || sender || timestamp_ms)
    pub signature: Vec<u8>,
}

impl SignedGossipMessage {
    /// Create a new signed gossip message
    pub fn new(event: DriveEvent, identity: &Identity) -> Self {
        let sender = identity.node_id();
        let timestamp_ms = Utc::now().timestamp_millis();
        
        // Create the message to sign: serialized event + sender bytes + timestamp
        let message_bytes = Self::create_signing_payload(&event, &sender, timestamp_ms);
        let signature = identity.sign(&message_bytes);
        
        Self {
            event,
            sender,
            timestamp_ms,
            signature: signature.to_bytes().to_vec(),
        }
    }
    
    /// Verify the signature of this message
    pub fn verify(&self) -> Result<(), GossipAuthError> {
        // Reconstruct the signed payload
        let message_bytes = Self::create_signing_payload(&self.event, &self.sender, self.timestamp_ms);
        
        // Parse the signature
        let signature_bytes: [u8; 64] = self.signature
            .clone()
            .try_into()
            .map_err(|_| GossipAuthError::InvalidSignature)?;
        let signature = Signature::from_bytes(&signature_bytes);
        
        // Parse the sender's public key
        let verifying_key = VerifyingKey::from_bytes(self.sender.as_bytes())
            .map_err(|_| GossipAuthError::InvalidSenderKey)?;
        
        // Verify the signature
        verifying_key
            .verify(&message_bytes, &signature)
            .map_err(|_| GossipAuthError::SignatureVerificationFailed)?;
        
        Ok(())
    }
    
    /// Check if the message is too old (replay attack prevention)
    /// Messages older than max_age_ms are considered stale
    pub fn is_stale(&self, max_age_ms: i64) -> bool {
        let now_ms = Utc::now().timestamp_millis();
        now_ms - self.timestamp_ms > max_age_ms
    }
    
    /// Create the payload that is signed
    fn create_signing_payload(event: &DriveEvent, sender: &NodeId, timestamp_ms: i64) -> Vec<u8> {
        let event_json = serde_json::to_vec(event).unwrap_or_default();
        let mut payload = Vec::with_capacity(event_json.len() + 32 + 8);
        payload.extend_from_slice(&event_json);
        payload.extend_from_slice(sender.as_bytes());
        payload.extend_from_slice(&timestamp_ms.to_le_bytes());
        payload
    }
}

/// Errors that can occur during gossip message authentication
#[derive(Debug, Clone)]
pub enum GossipAuthError {
    /// The signature bytes are malformed
    InvalidSignature,
    /// The sender's public key is invalid
    InvalidSenderKey,
    /// Signature verification failed
    SignatureVerificationFailed,
    /// Message is too old (possible replay attack)
    StaleMessage,
    /// Sender is not authorized for this action
    Unauthorized,
}

impl std::fmt::Display for GossipAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GossipAuthError::InvalidSignature => write!(f, "Invalid signature format"),
            GossipAuthError::InvalidSenderKey => write!(f, "Invalid sender public key"),
            GossipAuthError::SignatureVerificationFailed => write!(f, "Signature verification failed"),
            GossipAuthError::StaleMessage => write!(f, "Message is too old"),
            GossipAuthError::Unauthorized => write!(f, "Sender is not authorized"),
        }
    }
}

impl std::error::Error for GossipAuthError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Identity;

    #[test]
    fn test_event_serialization() {
        let identity = Identity::generate();
        let node_id = identity.node_id();

        let event = DriveEvent::FileChanged {
            path: PathBuf::from("test/file.txt"),
            hash: "abc123".to_string(),
            size: 1024,
            modified_by: node_id,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: DriveEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.event_type(), parsed.event_type());
    }

    #[test]
    fn test_event_dto_creation() {
        let identity = Identity::generate();
        let node_id = identity.node_id();

        let event = DriveEvent::UserJoined {
            user: node_id,
            timestamp: Utc::now(),
        };

        let dto = DriveEventDto::from_event("drive123", &event);

        assert_eq!(dto.drive_id, "drive123");
        assert_eq!(dto.event_type, "UserJoined");
    }
}
