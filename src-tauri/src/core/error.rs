//! Unified error types for production-ready error handling
//!
//! Provides structured error types with context for all operations.
//! Replaces ad-hoc String errors with proper error types.

use serde::Serialize;
use thiserror::Error;

/// Application-level errors for Tauri commands
#[derive(Error, Debug)]
pub enum AppError {
    // ========== Validation Errors ==========
    #[error("Validation error: {0}")]
    ValidationError(String),

    // ========== Drive Errors ==========
    #[error("Drive not found: {drive_id}")]
    DriveNotFound { drive_id: String },

    #[error("Drive already exists: {name}")]
    DriveAlreadyExists { name: String },

    #[error("Invalid drive ID format: {id}")]
    InvalidDriveId { id: String },

    // ========== Path Errors ==========
    #[error("Path does not exist: {path}")]
    PathNotFound { path: String },

    #[error("Path is not a directory: {path}")]
    NotADirectory { path: String },

    #[error("Path is not a file: {path}")]
    NotAFile { path: String },

    #[error("Path traversal detected: {path}")]
    PathTraversal { path: String },

    #[error("Path outside drive root: {path}")]
    PathOutsideDrive { path: String },

    #[error("Invalid path: {path} - {reason}")]
    InvalidPath { path: String, reason: String },

    // ========== Identity Errors ==========
    #[error("Identity not initialized")]
    IdentityNotInitialized,

    #[error("Failed to load identity: {0}")]
    IdentityLoadFailed(String),

    // ========== Permission Errors ==========
    #[error("Insufficient permission: {required} required for {operation}")]
    InsufficientPermission { required: String, operation: String },

    #[error("Cannot revoke owner's access")]
    CannotRevokeOwner,

    #[error("Access denied: {reason}")]
    AccessDenied { reason: String },

    // ========== Sync Errors ==========
    #[error("Sync engine not initialized")]
    SyncNotInitialized,

    #[error("File watcher not initialized")]
    WatcherNotInitialized,

    #[error("File transfer not initialized")]
    TransferNotInitialized,

    #[error("Event broadcaster not initialized")]
    BroadcasterNotInitialized,

    #[error("Sync failed: {0}")]
    SyncFailed(String),

    // ========== Lock Errors ==========
    #[error("File locked by another user: {holder}")]
    FileLocked { path: String, holder: String },

    #[error("Lock not found: {path}")]
    LockNotFound { path: String },

    #[error("Lock expired: {path}")]
    LockExpired { path: String },

    // ========== Transfer Errors ==========
    #[error("Transfer failed: {0}")]
    TransferFailed(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Transfer not found: {id}")]
    TransferNotFound { id: String },

    // ========== Token Errors ==========
    #[error("Invalid token format")]
    InvalidTokenFormat,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Token already used")]
    TokenAlreadyUsed,

    // ========== Validation Errors ==========
    #[error("Validation failed: {field} - {reason}")]
    ValidationFailed { field: String, reason: String },

    #[error("Name too long: max {max} characters")]
    NameTooLong { max: usize },

    #[error("Name cannot be empty")]
    NameEmpty,

    #[error("Name contains invalid characters")]
    NameInvalidChars,

    // ========== Database Errors ==========
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    // ========== Internal Errors ==========
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Rate limited: try again in {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },
}

impl AppError {
    /// Get an error code for frontend consumption
    pub fn code(&self) -> &'static str {
        match self {
            AppError::ValidationError(_) => "VALIDATION_ERROR",
            AppError::DriveNotFound { .. } => "DRIVE_NOT_FOUND",
            AppError::DriveAlreadyExists { .. } => "DRIVE_EXISTS",
            AppError::InvalidDriveId { .. } => "INVALID_DRIVE_ID",
            AppError::PathNotFound { .. } => "PATH_NOT_FOUND",
            AppError::NotADirectory { .. } => "NOT_A_DIRECTORY",
            AppError::NotAFile { .. } => "NOT_A_FILE",
            AppError::PathTraversal { .. } => "PATH_TRAVERSAL",
            AppError::PathOutsideDrive { .. } => "PATH_OUTSIDE_DRIVE",
            AppError::InvalidPath { .. } => "INVALID_PATH",
            AppError::IdentityNotInitialized => "IDENTITY_NOT_INIT",
            AppError::IdentityLoadFailed(_) => "IDENTITY_LOAD_FAILED",
            AppError::InsufficientPermission { .. } => "PERMISSION_DENIED",
            AppError::CannotRevokeOwner => "CANNOT_REVOKE_OWNER",
            AppError::AccessDenied { .. } => "ACCESS_DENIED",
            AppError::SyncNotInitialized => "SYNC_NOT_INIT",
            AppError::WatcherNotInitialized => "WATCHER_NOT_INIT",
            AppError::TransferNotInitialized => "TRANSFER_NOT_INIT",
            AppError::BroadcasterNotInitialized => "BROADCASTER_NOT_INIT",
            AppError::SyncFailed(_) => "SYNC_FAILED",
            AppError::FileLocked { .. } => "FILE_LOCKED",
            AppError::LockNotFound { .. } => "LOCK_NOT_FOUND",
            AppError::LockExpired { .. } => "LOCK_EXPIRED",
            AppError::TransferFailed(_) => "TRANSFER_FAILED",
            AppError::InvalidHash(_) => "INVALID_HASH",
            AppError::TransferNotFound { .. } => "TRANSFER_NOT_FOUND",
            AppError::InvalidTokenFormat => "INVALID_TOKEN",
            AppError::TokenExpired => "TOKEN_EXPIRED",
            AppError::InvalidSignature => "INVALID_SIGNATURE",
            AppError::TokenAlreadyUsed => "TOKEN_USED",
            AppError::ValidationFailed { .. } => "VALIDATION_FAILED",
            AppError::NameTooLong { .. } => "NAME_TOO_LONG",
            AppError::NameEmpty => "NAME_EMPTY",
            AppError::NameInvalidChars => "NAME_INVALID_CHARS",
            AppError::DatabaseError(_) => "DATABASE_ERROR",
            AppError::SerializationError(_) => "SERIALIZATION_ERROR",
            AppError::Internal(_) => "INTERNAL_ERROR",
            AppError::RateLimited { .. } => "RATE_LIMITED",
        }
    }

    /// Check if this error is recoverable by retry
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AppError::SyncFailed(_)
                | AppError::TransferFailed(_)
                | AppError::RateLimited { .. }
                | AppError::DatabaseError(_)
        )
    }
}

/// Serializable error response for frontend
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl From<&AppError> for ErrorResponse {
    fn from(error: &AppError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
            retryable: error.is_retryable(),
        }
    }
}

// Allow AppError to be returned from Tauri commands
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        // Include error code for structured frontend handling
        format!("[{}] {}", error.code(), error)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        AppError::Internal(error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Internal(format!("I/O error: {}", error))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::SerializationError(error.to_string())
    }
}

impl From<hex::FromHexError> for AppError {
    fn from(_: hex::FromHexError) -> Self {
        AppError::InvalidDriveId {
            id: "invalid hex".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = AppError::DriveNotFound {
            drive_id: "abc123".to_string(),
        };
        assert_eq!(err.code(), "DRIVE_NOT_FOUND");
        assert!(!err.is_retryable());

        let err = AppError::RateLimited {
            retry_after_secs: 5,
        };
        assert_eq!(err.code(), "RATE_LIMITED");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_error_response_serialization() {
        let err = AppError::PathTraversal {
            path: "../etc/passwd".to_string(),
        };
        let response = ErrorResponse::from(&err);
        assert_eq!(response.code, "PATH_TRAVERSAL");
        assert!(!response.retryable);
    }
}
