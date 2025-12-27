//! Tauri commands for file locking
//!
//! Provides commands for acquiring, releasing, and querying file locks.
//!
//! # Security
//! - Validates drive IDs before any lock operations
//! - Validates paths to prevent directory traversal attacks
//! - Enforces ACL permission checks for privileged operations

use crate::commands::security::SecurityStore;
use crate::core::error::AppError;
use crate::core::validation::{validate_drive_id, validate_path};
use crate::core::{DriveEvent, FileLock, FileLockDto, LockManager, LockResult, LockType};
use crate::crypto::Permission;
use crate::state::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

/// Parse and validate drive ID
fn parse_drive_id(drive_id: &str) -> Result<crate::core::drive::DriveId, String> {
    validate_drive_id(drive_id).map_err(|e| e.to_string())?;
    crate::core::drive::DriveId::from_hex(drive_id).map_err(|e| e.to_string())
}

/// DTO for lock acquisition result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcquireLockResult {
    pub success: bool,
    pub lock: Option<FileLockDto>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

/// Acquire a lock on a file
///
/// # Security
/// - Validates path is within drive root
#[tauri::command]
pub async fn acquire_lock(
    drive_id: String,
    path: String,
    lock_type: String,
    state: State<'_, AppState>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<AcquireLockResult, String> {
    let id = parse_drive_id(&drive_id)?;

    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);

    let lock_type = match lock_type.as_str() {
        "exclusive" => LockType::Exclusive,
        _ => LockType::Advisory,
    };

    let result = lock_manager
        .acquire_lock(&drive_id, validated_path.clone(), lock_type)
        .await;
    let node_id = lock_manager.node_id();

    match result {
        LockResult::Acquired(lock) => {
            // Broadcast lock event via gossip
            broadcast_lock_acquired(&state, &drive_id, &lock).await;

            tracing::info!(
                drive_id = %drive_id,
                path = %path,
                lock_type = ?lock_type,
                "Lock acquired"
            );

            Ok(AcquireLockResult {
                success: true,
                lock: Some(FileLockDto::from_lock(&lock, node_id)),
                error: None,
                warning: None,
            })
        }
        LockResult::AcquiredWithWarning { lock, warning } => {
            broadcast_lock_acquired(&state, &drive_id, &lock).await;

            tracing::info!(
                drive_id = %drive_id,
                path = %path,
                warning = %warning,
                "Lock acquired with warning"
            );

            Ok(AcquireLockResult {
                success: true,
                lock: Some(FileLockDto::from_lock(&lock, node_id)),
                error: None,
                warning: Some(warning),
            })
        }
        LockResult::Denied {
            existing_lock,
            reason,
        } => {
            tracing::debug!(
                drive_id = %drive_id,
                path = %path,
                reason = %reason,
                "Lock denied"
            );

            Ok(AcquireLockResult {
                success: false,
                lock: Some(FileLockDto::from_lock(&existing_lock, node_id)),
                error: Some(reason),
                warning: None,
            })
        }
    }
}

/// Release a lock on a file
#[tauri::command]
pub async fn release_lock(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<bool, String> {
    let id = parse_drive_id(&drive_id)?;

    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);

    if let Some(released) = lock_manager.release_lock(&drive_id, &validated_path).await {
        // Broadcast lock release via gossip
        broadcast_lock_released(&state, &drive_id, &released).await;
        tracing::info!(drive_id = %drive_id, path = %path, "Lock released");
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Get lock status for a specific file
#[tauri::command]
pub async fn get_lock_status(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<Option<FileLockDto>, String> {
    let id = parse_drive_id(&drive_id)?;

    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);

    let node_id = lock_manager.node_id();

    Ok(lock_manager
        .get_lock(&drive_id, &validated_path)
        .await
        .map(|lock| FileLockDto::from_lock(&lock, node_id)))
}

/// List all locks for a drive
#[tauri::command]
pub async fn list_locks(
    drive_id: String,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<Vec<FileLockDto>, String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;

    let node_id = lock_manager.node_id();
    let locks = lock_manager.list_locks(&drive_id).await;

    Ok(locks
        .iter()
        .map(|lock| FileLockDto::from_lock(lock, node_id))
        .collect())
}

/// Extend an existing lock
#[tauri::command]
pub async fn extend_lock(
    drive_id: String,
    path: String,
    duration_mins: i64,
    state: State<'_, AppState>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<Option<FileLockDto>, String> {
    let id = parse_drive_id(&drive_id)?;

    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);

    // Validate duration (1 minute to 24 hours)
    if !(1..=1440).contains(&duration_mins) {
        return Err(AppError::ValidationError(
            "Lock duration must be between 1 and 1440 minutes".to_string(),
        )
        .to_string());
    }

    let node_id = lock_manager.node_id();

    if let Some(lock) = lock_manager
        .extend_lock(&drive_id, &validated_path, duration_mins)
        .await
    {
        // Broadcast updated lock
        broadcast_lock_acquired(&state, &drive_id, &lock).await;
        tracing::info!(
            drive_id = %drive_id,
            path = %path,
            duration_mins = duration_mins,
            "Lock extended"
        );
        Ok(Some(FileLockDto::from_lock(&lock, node_id)))
    } else {
        Ok(None)
    }
}

/// Force release a lock (admin only)
///
/// # Security
/// - Validates path is within drive root
/// - Enforces ACL permission check (requires Admin permission)
#[tauri::command]
pub async fn force_release_lock(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<bool, String> {
    let id = parse_drive_id(&drive_id)?;

    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    let owner_hex = drive.owner.to_hex();
    drop(drives);

    // Get caller identity and verify admin permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();

    // Enforce ACL permission check (requires Admin to force release locks)
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, "/", Permission::Admin) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to force release lock"
        );
        return Err(AppError::AccessDenied {
            reason: "Only admin can force release locks".to_string(),
        }
        .to_string());
    }

    let manager = lock_manager.get_drive_locks(&drive_id).await;

    if let Some(released) = manager.force_release(&validated_path).await {
        broadcast_lock_released(&state, &drive_id, &released).await;
        tracing::warn!(
            drive_id = %drive_id,
            path = %path,
            holder = %released.holder,
            "Lock force released"
        );
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Broadcast lock acquired event via gossip
async fn broadcast_lock_acquired(state: &AppState, drive_id: &str, lock: &FileLock) {
    if let Some(ref broadcaster) = state.event_broadcaster {
        if let Ok(id) = crate::core::drive::DriveId::from_hex(drive_id) {
            let event = DriveEvent::FileLockAcquired {
                path: lock.path.clone(),
                holder: lock.holder,
                lock_type: match lock.lock_type {
                    LockType::Advisory => "advisory".to_string(),
                    LockType::Exclusive => "exclusive".to_string(),
                },
                expires_at: lock.expires_at,
                timestamp: Utc::now(),
            };

            if let Err(e) = broadcaster.broadcast(&id, event).await {
                tracing::warn!("Failed to broadcast lock acquired: {}", e);
            }
        }
    }
}

/// Broadcast lock released event via gossip
async fn broadcast_lock_released(state: &AppState, drive_id: &str, lock: &FileLock) {
    if let Some(ref broadcaster) = state.event_broadcaster {
        if let Ok(id) = crate::core::drive::DriveId::from_hex(drive_id) {
            let event = DriveEvent::FileLockReleased {
                path: lock.path.clone(),
                holder: lock.holder,
                timestamp: Utc::now(),
            };

            if let Err(e) = broadcaster.broadcast(&id, event).await {
                tracing::warn!("Failed to broadcast lock released: {}", e);
            }
        }
    }
}
