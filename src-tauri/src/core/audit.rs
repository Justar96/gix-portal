//! Security audit logging for drive operations
//!
//! Provides persistent audit trail for security-sensitive events including:
//! - Identity and authentication events
//! - Drive access and permission changes
//! - File operations
//! - Invite generation and acceptance
//! - Lock force releases

use crate::storage::Database;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Audit event types for security logging
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEvent {
    // ============================================================================
    // Identity Events
    // ============================================================================
    /// A new identity was created
    IdentityCreated { node_id: String },

    // ============================================================================
    // Drive Access Events
    // ============================================================================
    /// A drive was accessed
    DriveAccessed {
        drive_id: String,
        user_id: String,
        operation: String,
    },

    /// Access to a drive was denied
    AccessDenied {
        drive_id: String,
        user_id: String,
        path: String,
        reason: String,
    },

    // ============================================================================
    // Permission Events
    // ============================================================================
    /// Permission was granted to a user
    PermissionGranted {
        drive_id: String,
        user_id: String,
        permission: String,
        granted_by: String,
    },

    /// Permission was revoked from a user
    PermissionRevoked {
        drive_id: String,
        user_id: String,
        revoked_by: String,
    },

    // ============================================================================
    // Invite Events
    // ============================================================================
    /// An invite token was created
    InviteCreated {
        drive_id: String,
        token_id: String,
        permission: String,
        created_by: String,
        expires_at: DateTime<Utc>,
    },

    /// An invite was accepted
    InviteAccepted {
        drive_id: String,
        token_id: String,
        user_id: String,
    },

    /// An invite was revoked
    InviteRevoked {
        drive_id: String,
        token_id: String,
        revoked_by: String,
    },

    // ============================================================================
    // File Events
    // ============================================================================
    /// A file was read
    FileRead {
        drive_id: String,
        path: String,
        user_id: String,
    },

    /// A file was written
    FileWritten {
        drive_id: String,
        path: String,
        user_id: String,
        size: u64,
    },

    /// A file was deleted
    FileDeleted {
        drive_id: String,
        path: String,
        user_id: String,
    },

    /// A file was renamed
    FileRenamed {
        drive_id: String,
        old_path: String,
        new_path: String,
        user_id: String,
    },

    // ============================================================================
    // Lock Events
    // ============================================================================
    /// A lock was force released by an admin
    LockForceReleased {
        drive_id: String,
        path: String,
        by_user: String,
        lock_holder: String,
    },
}

impl AuditEvent {
    /// Get the event type name
    #[allow(dead_code)]
    pub fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::IdentityCreated { .. } => "identity_created",
            AuditEvent::DriveAccessed { .. } => "drive_accessed",
            AuditEvent::AccessDenied { .. } => "access_denied",
            AuditEvent::PermissionGranted { .. } => "permission_granted",
            AuditEvent::PermissionRevoked { .. } => "permission_revoked",
            AuditEvent::InviteCreated { .. } => "invite_created",
            AuditEvent::InviteAccepted { .. } => "invite_accepted",
            AuditEvent::InviteRevoked { .. } => "invite_revoked",
            AuditEvent::FileRead { .. } => "file_read",
            AuditEvent::FileWritten { .. } => "file_written",
            AuditEvent::FileDeleted { .. } => "file_deleted",
            AuditEvent::FileRenamed { .. } => "file_renamed",
            AuditEvent::LockForceReleased { .. } => "lock_force_released",
        }
    }

    /// Get the drive ID if this event is associated with a drive
    #[allow(dead_code)]
    pub fn drive_id(&self) -> Option<&str> {
        match self {
            AuditEvent::IdentityCreated { .. } => None,
            AuditEvent::DriveAccessed { drive_id, .. }
            | AuditEvent::AccessDenied { drive_id, .. }
            | AuditEvent::PermissionGranted { drive_id, .. }
            | AuditEvent::PermissionRevoked { drive_id, .. }
            | AuditEvent::InviteCreated { drive_id, .. }
            | AuditEvent::InviteAccepted { drive_id, .. }
            | AuditEvent::InviteRevoked { drive_id, .. }
            | AuditEvent::FileRead { drive_id, .. }
            | AuditEvent::FileWritten { drive_id, .. }
            | AuditEvent::FileDeleted { drive_id, .. }
            | AuditEvent::FileRenamed { drive_id, .. }
            | AuditEvent::LockForceReleased { drive_id, .. } => Some(drive_id),
        }
    }

    /// Get the user ID if this event is associated with a user
    #[allow(dead_code)]
    pub fn user_id(&self) -> Option<&str> {
        match self {
            AuditEvent::IdentityCreated { node_id } => Some(node_id),
            AuditEvent::DriveAccessed { user_id, .. }
            | AuditEvent::AccessDenied { user_id, .. }
            | AuditEvent::PermissionGranted { user_id, .. }
            | AuditEvent::PermissionRevoked { user_id, .. }
            | AuditEvent::InviteAccepted { user_id, .. }
            | AuditEvent::FileRead { user_id, .. }
            | AuditEvent::FileWritten { user_id, .. }
            | AuditEvent::FileDeleted { user_id, .. }
            | AuditEvent::FileRenamed { user_id, .. } => Some(user_id),
            AuditEvent::InviteCreated { created_by, .. } => Some(created_by),
            AuditEvent::InviteRevoked { revoked_by, .. } => Some(revoked_by),
            AuditEvent::LockForceReleased { by_user, .. } => Some(by_user),
        }
    }
}

/// A persisted audit log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID (auto-incrementing)
    pub id: u64,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// The event type name
    pub event_type: String,
    /// Drive ID (if applicable)
    pub drive_id: Option<String>,
    /// User ID (if applicable)
    pub user_id: Option<String>,
    /// Full event details
    pub event: AuditEvent,
}

/// Filter criteria for querying audit logs
#[derive(Clone, Debug, Default)]
pub struct AuditFilter {
    /// Filter by drive ID
    pub drive_id: Option<String>,
    /// Filter by event type
    pub event_type: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Only include events after this timestamp (Unix ms)
    pub since: Option<i64>,
    /// Only include events before this timestamp (Unix ms)
    pub until: Option<i64>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Number of results to skip (for pagination)
    pub offset: Option<usize>,
}

/// Error types for audit operations
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Audit logger for persisting security events
pub struct AuditLogger {
    db: Arc<Database>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Log a security event
    #[allow(dead_code)]
    pub async fn log(&self, event: AuditEvent) -> Result<u64, AuditError> {
        let timestamp = Utc::now();
        let event_type = event.event_type().to_string();
        let drive_id = event.drive_id().map(String::from);
        let user_id = event.user_id().map(String::from);

        let entry = AuditEntry {
            id: 0, // Will be assigned by database
            timestamp,
            event_type: event_type.clone(),
            drive_id: drive_id.clone(),
            user_id: user_id.clone(),
            event,
        };

        let entry_bytes = serde_json::to_vec(&entry)?;
        let id = self.db.append_audit_log(&entry_bytes)?;

        tracing::debug!(
            id = id,
            event_type = %event_type,
            drive_id = ?drive_id,
            user_id = ?user_id,
            "Audit event logged"
        );

        Ok(id)
    }

    /// Query audit logs with optional filters
    pub async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>, AuditError> {
        let entries = self.db.query_audit_log(
            filter.since,
            filter.until,
            filter.limit.unwrap_or(100),
            filter.offset.unwrap_or(0),
        )?;

        let mut results = Vec::new();
        for (id, bytes) in entries {
            if let Ok(mut entry) = serde_json::from_slice::<AuditEntry>(&bytes) {
                entry.id = id;

                // Apply filters
                if let Some(ref drive_id) = filter.drive_id {
                    if entry.drive_id.as_ref() != Some(drive_id) {
                        continue;
                    }
                }
                if let Some(ref event_type) = filter.event_type {
                    if &entry.event_type != event_type {
                        continue;
                    }
                }
                if let Some(ref user_id) = filter.user_id {
                    if entry.user_id.as_ref() != Some(user_id) {
                        continue;
                    }
                }

                results.push(entry);
            }
        }

        Ok(results)
    }

    /// Get the total count of audit entries
    pub async fn count(&self) -> Result<u64, AuditError> {
        Ok(self.db.count_audit_log()?)
    }

    /// Get recent security events for a specific drive
    pub async fn get_drive_events(
        &self,
        drive_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        self.query(AuditFilter {
            drive_id: Some(drive_id.to_string()),
            limit: Some(limit),
            ..Default::default()
        })
        .await
    }

    /// Get recent security events for a specific user
    #[allow(dead_code)]
    pub async fn get_user_events(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        self.query(AuditFilter {
            user_id: Some(user_id.to_string()),
            limit: Some(limit),
            ..Default::default()
        })
        .await
    }

    /// Get access denied events (for security monitoring)
    pub async fn get_denied_access_events(
        &self,
        limit: usize,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        self.query(AuditFilter {
            event_type: Some("access_denied".to_string()),
            limit: Some(limit),
            ..Default::default()
        })
        .await
    }
}

/// DTO for returning audit entries to the frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntryDto {
    pub id: u64,
    pub timestamp: i64,
    pub event_type: String,
    pub drive_id: Option<String>,
    pub user_id: Option<String>,
    pub details: serde_json::Value,
}

impl From<AuditEntry> for AuditEntryDto {
    fn from(entry: AuditEntry) -> Self {
        Self {
            id: entry.id,
            timestamp: entry.timestamp.timestamp_millis(),
            event_type: entry.event_type,
            drive_id: entry.drive_id,
            user_id: entry.user_id,
            details: serde_json::to_value(&entry.event).unwrap_or_default(),
        }
    }
}
