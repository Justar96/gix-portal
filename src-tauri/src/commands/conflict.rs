//! Tauri commands for conflict resolution
//!
//! Provides commands for listing, viewing, and resolving file conflicts.

use crate::core::{ConflictManager, FileConflictDto, ResolutionStrategy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

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
    let conflicts = conflict_manager.list_conflicts(&drive_id).await;
    Ok(conflicts.iter().map(FileConflictDto::from).collect())
}

/// Get a specific conflict by path
#[tauri::command]
pub async fn get_conflict(
    drive_id: String,
    path: String,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<Option<FileConflictDto>, String> {
    let manager = conflict_manager.get_drive_conflicts(&drive_id).await;
    let path = PathBuf::from(&path);

    Ok(manager.get_conflict(&path).await.map(|c| FileConflictDto::from(&c)))
}

/// Resolve a conflict with the given strategy
#[tauri::command]
pub async fn resolve_conflict(
    drive_id: String,
    path: String,
    strategy: String,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<Option<FileConflictDto>, String> {
    let path = PathBuf::from(&path);
    let strategy = match strategy.to_lowercase().as_str() {
        "keeplocal" | "keep_local" | "local" => ResolutionStrategy::KeepLocal,
        "keepremote" | "keep_remote" | "remote" => ResolutionStrategy::KeepRemote,
        "keepboth" | "keep_both" | "both" => ResolutionStrategy::KeepBoth,
        "manualmerge" | "manual_merge" | "merge" => ResolutionStrategy::ManualMerge,
        _ => return Err(format!("Invalid resolution strategy: {}", strategy)),
    };

    let resolved = conflict_manager
        .resolve_conflict(&drive_id, &path, strategy)
        .await;

    Ok(resolved.map(|c| FileConflictDto::from(&c)))
}

/// Get total conflict count for a drive
#[tauri::command]
pub async fn get_conflict_count(
    drive_id: String,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<usize, String> {
    let manager = conflict_manager.get_drive_conflicts(&drive_id).await;
    Ok(manager.conflict_count().await)
}

/// Dismiss/clear a conflict without resolving (user accepts current state)
#[tauri::command]
pub async fn dismiss_conflict(
    drive_id: String,
    path: String,
    conflict_manager: State<'_, Arc<ConflictManager>>,
) -> Result<bool, String> {
    let path = PathBuf::from(&path);
    let manager = conflict_manager.get_drive_conflicts(&drive_id).await;

    // Resolve with KeepLocal to dismiss (accepts current local state)
    let resolved = manager
        .resolve_conflict(&path, ResolutionStrategy::KeepLocal)
        .await;

    Ok(resolved.is_some())
}
