//! Tauri commands for conflict resolution
//!
//! Provides commands for listing, viewing, and resolving file conflicts.
//! 
//! # Security
//! - Validates drive IDs before operations
//! - Validates paths to prevent directory traversal attacks

use crate::core::error::AppError;
use crate::core::validation::{validate_drive_id, validate_path};
use crate::core::{ConflictManager, FileConflictDto, ResolutionStrategy};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

/// Parse and validate drive ID
fn parse_drive_id(drive_id: &str) -> Result<crate::core::drive::DriveId, String> {
    validate_drive_id(drive_id).map_err(|e| e.to_string())?;
    crate::core::drive::DriveId::from_hex(drive_id).map_err(|e| e.to_string())
}

/// DTO for resolution request
#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ResolveConflictRequest {
    pub drive_id: String,
    pub path: String,
    pub strategy: String,
}

/// List all conflicts for a drive
#[tauri::command]
pub async fn list_conflicts(
    drive_id: String,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<Vec<FileConflictDto>, String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    let conflicts = conflict_manager.list_conflicts(&drive_id).await;
    Ok(conflicts.iter().map(FileConflictDto::from).collect())
}

/// Get a specific conflict by path
#[tauri::command]
pub async fn get_conflict(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<Option<FileConflictDto>, String> {
    let id = parse_drive_id(&drive_id)?;
    
    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound { drive_id: drive_id.clone() }.to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);
    
    let manager = conflict_manager.get_drive_conflicts(&drive_id).await;

    Ok(manager.get_conflict(&validated_path).await.map(|c| FileConflictDto::from(&c)))
}

/// Resolve a conflict with the given strategy
#[tauri::command]
pub async fn resolve_conflict(
    drive_id: String,
    path: String,
    strategy: String,
    state: State<'_, AppState>,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<Option<FileConflictDto>, String> {
    let id = parse_drive_id(&drive_id)?;
    
    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound { drive_id: drive_id.clone() }.to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);
    
    let strategy = match strategy.to_lowercase().as_str() {
        "keeplocal" | "keep_local" | "local" => ResolutionStrategy::KeepLocal,
        "keepremote" | "keep_remote" | "remote" => ResolutionStrategy::KeepRemote,
        "keepboth" | "keep_both" | "both" => ResolutionStrategy::KeepBoth,
        "manualmerge" | "manual_merge" | "merge" => ResolutionStrategy::ManualMerge,
        _ => return Err(AppError::ValidationError(
            format!("Invalid resolution strategy: {}. Use: keeplocal, keepremote, keepboth, or manualmerge", strategy)
        ).to_string()),
    };

    let resolved = conflict_manager
        .resolve_conflict(&drive_id, &validated_path, strategy)
        .await;

    if resolved.is_some() {
        tracing::info!(
            drive_id = %drive_id,
            path = %path,
            strategy = ?strategy,
            "Conflict resolved"
        );
    }

    Ok(resolved.map(|c| FileConflictDto::from(&c)))
}

/// Get total conflict count for a drive
#[tauri::command]
pub async fn get_conflict_count(
    drive_id: String,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<usize, String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    let manager = conflict_manager.get_drive_conflicts(&drive_id).await;
    Ok(manager.conflict_count().await)
}

/// Dismiss/clear a conflict without resolving (user accepts current state)
#[tauri::command]
pub async fn dismiss_conflict(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<bool, String> {
    let id = parse_drive_id(&drive_id)?;
    
    // Validate path against drive root
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound { drive_id: drive_id.clone() }.to_string()
    })?;
    let validated_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;
    drop(drives);
    
    let manager = conflict_manager.get_drive_conflicts(&drive_id).await;

    // Resolve with KeepLocal to dismiss (accepts current local state)
    let resolved = manager
        .resolve_conflict(&validated_path, ResolutionStrategy::KeepLocal)
        .await;

    if resolved.is_some() {
        tracing::info!(drive_id = %drive_id, path = %path, "Conflict dismissed");
    }

    Ok(resolved.is_some())
}
