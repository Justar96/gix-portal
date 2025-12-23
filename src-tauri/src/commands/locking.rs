//! Tauri commands for file locking
//!
//! Provides commands for acquiring, releasing, and querying file locks.

use crate::core::{DriveEvent, FileLock, FileLockDto, LockManager, LockResult, LockType};
use crate::state::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

/// DTO for lock acquisition result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcquireLockResult {
    pub success: bool,
    pub lock: Option<FileLockDto>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

/// Acquire a lock on a file
#[tauri::command]
pub async fn acquire_lock(
    drive_id: String,
    path: String,
    lock_type: String,
    state: State<'_, AppState>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<AcquireLockResult, String> {
    let path = PathBuf::from(&path);
    let lock_type = match lock_type.as_str() {
        "exclusive" => LockType::Exclusive,
        _ => LockType::Advisory,
    };

    let result = lock_manager.acquire_lock(&drive_id, path.clone(), lock_type).await;
    let node_id = lock_manager.node_id();

    match result {
        LockResult::Acquired(lock) => {
            // Broadcast lock event via gossip
            broadcast_lock_acquired(&state, &drive_id, &lock).await;

            Ok(AcquireLockResult {
                success: true,
                lock: Some(FileLockDto::from_lock(&lock, node_id)),
                error: None,
                warning: None,
            })
        }
        LockResult::AcquiredWithWarning { lock, warning } => {
            broadcast_lock_acquired(&state, &drive_id, &lock).await;

            Ok(AcquireLockResult {
                success: true,
                lock: Some(FileLockDto::from_lock(&lock, node_id)),
                error: None,
                warning: Some(warning),
            })
        }
        LockResult::Denied { existing_lock, reason } => {
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
    let path = PathBuf::from(&path);

    if let Some(released) = lock_manager.release_lock(&drive_id, &path).await {
        // Broadcast lock release via gossip
        broadcast_lock_released(&state, &drive_id, &released).await;
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
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<Option<FileLockDto>, String> {
    let path = PathBuf::from(&path);
    let node_id = lock_manager.node_id();

    Ok(lock_manager
        .get_lock(&drive_id, &path)
        .await
        .map(|lock| FileLockDto::from_lock(&lock, node_id)))
}

/// List all locks for a drive
#[tauri::command]
pub async fn list_locks(
    drive_id: String,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<Vec<FileLockDto>, String> {
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
    let path = PathBuf::from(&path);
    let node_id = lock_manager.node_id();

    if let Some(lock) = lock_manager.extend_lock(&drive_id, &path, duration_mins).await {
        // Broadcast updated lock
        broadcast_lock_acquired(&state, &drive_id, &lock).await;
        Ok(Some(FileLockDto::from_lock(&lock, node_id)))
    } else {
        Ok(None)
    }
}

/// Force release a lock (admin only)
#[tauri::command]
pub async fn force_release_lock(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    lock_manager: State<'_, Arc<LockManager>>,
) -> Result<bool, String> {
    let path = PathBuf::from(&path);
    let manager = lock_manager.get_drive_locks(&drive_id).await;

    if let Some(released) = manager.force_release(&path).await {
        broadcast_lock_released(&state, &drive_id, &released).await;
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
