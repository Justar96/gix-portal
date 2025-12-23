//! Drive events for real-time synchronization
//!
//! Events are broadcast via gossip protocol and used for:
//! - Notifying peers of file changes
//! - Tracking user presence (join/leave)
//! - Sync progress reporting

use crate::crypto::NodeId;
use chrono::{DateTime, Utc};
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
