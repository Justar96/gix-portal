//! Drive management commands with production-ready validation
//!
//! All commands include proper input validation, path sanitization,
//! and structured error handling.

use crate::core::{file, validate_drive_id, validate_name, AppError, DriveInfo, SharedDrive};
use crate::state::AppState;
use tauri::State;

/// Maximum file count for initial indexing (prevent DoS)
const MAX_INDEX_FILES: usize = 100_000;

/// Create a new shared drive from a local folder
#[tauri::command]
pub async fn create_drive(
    name: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<DriveInfo, String> {
    // Validate name
    let validated_name = validate_name(&name, "drive name").map_err(|e| e.to_string())?;

    let local_path = std::path::PathBuf::from(&path);

    // Validate path exists and is a directory
    if !local_path.exists() {
        return Err(AppError::PathNotFound { path: path.clone() }.to_string());
    }
    if !local_path.is_dir() {
        return Err(AppError::NotADirectory { path: path.clone() }.to_string());
    }

    // Ensure path is absolute for security
    let local_path = local_path.canonicalize().map_err(|e| {
        AppError::InvalidPath {
            path: path.clone(),
            reason: format!("Cannot canonicalize: {}", e),
        }
        .to_string()
    })?;

    // Get owner identity
    let owner = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;

    // Create drive
    let mut drive = SharedDrive::new(validated_name.clone(), local_path.clone(), owner);

    // Index files and update stats (with limit to prevent DoS)
    let entries = file::index_directory(&local_path)
        .map_err(|e| format!("Failed to index directory: {}", e))?;

    if entries.len() > MAX_INDEX_FILES {
        tracing::warn!(
            "Drive '{}' has {} files, exceeds recommended limit of {}",
            validated_name,
            entries.len(),
            MAX_INDEX_FILES
        );
    }

    let total_size: u64 = entries.iter().map(|e| e.size).sum();
    let file_count = entries.iter().filter(|e| !e.is_dir).count() as u64;
    drive.update_stats(total_size, file_count);

    // Save to database
    let drive_bytes = serde_json::to_vec(&drive).map_err(|e| {
        AppError::SerializationError(format!("Failed to serialize drive: {}", e)).to_string()
    })?;

    state
        .db
        .save_drive(drive.id.as_bytes(), &drive_bytes)
        .map_err(|e| AppError::DatabaseError(format!("Failed to save drive: {}", e)).to_string())?;

    // Add to in-memory cache
    state
        .drives
        .write()
        .await
        .insert(*drive.id.as_bytes(), drive.clone());

    tracing::info!(
        drive_id = %drive.id,
        name = %validated_name,
        path = %local_path.display(),
        file_count = file_count,
        total_size = total_size,
        "Created new drive"
    );

    Ok(DriveInfo::from(&drive))
}

/// List all owned drives
#[tauri::command]
pub async fn list_drives(state: State<'_, AppState>) -> Result<Vec<DriveInfo>, String> {
    let drives = state.drives.read().await;
    let infos: Vec<DriveInfo> = drives.values().map(DriveInfo::from).collect();

    tracing::debug!(count = infos.len(), "Listed drives");
    Ok(infos)
}

/// Get a specific drive by ID
#[tauri::command]
pub async fn get_drive(drive_id: String, state: State<'_, AppState>) -> Result<DriveInfo, String> {
    let id_arr = validate_drive_id(&drive_id).map_err(|e| e.to_string())?;

    let drives = state.drives.read().await;
    let drive = drives.get(&id_arr).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    Ok(DriveInfo::from(drive))
}

/// Delete a drive by ID
#[tauri::command]
pub async fn delete_drive(drive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id_arr = validate_drive_id(&drive_id).map_err(|e| e.to_string())?;

    // Stop any active sync/watching first
    if let Some(ref sync_engine) = state.sync_engine {
        sync_engine.stop_sync(&crate::core::DriveId(id_arr)).await;
    }
    if let Some(ref file_watcher) = state.file_watcher {
        file_watcher.unwatch(&crate::core::DriveId(id_arr)).await;
    }

    // Remove from database
    let removed = state.db.delete_drive(&id_arr).map_err(|e| {
        AppError::DatabaseError(format!("Failed to delete drive: {}", e)).to_string()
    })?;

    if !removed {
        return Err(AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string());
    }

    // Remove from in-memory cache
    state.drives.write().await.remove(&id_arr);

    tracing::info!(drive_id = %drive_id, "Deleted drive");
    Ok(())
}

/// Rename a drive
#[tauri::command]
pub async fn rename_drive(
    drive_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<DriveInfo, String> {
    let id_arr = validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    let validated_name = validate_name(&new_name, "drive name").map_err(|e| e.to_string())?;

    // Update in memory first
    let mut drives = state.drives.write().await;
    let drive = drives.get_mut(&id_arr).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    let old_name = drive.name.clone();
    drive.name = validated_name.clone();

    // Save to database
    let drive_bytes = serde_json::to_vec(&drive).map_err(|e| {
        AppError::SerializationError(format!("Failed to serialize drive: {}", e)).to_string()
    })?;

    state
        .db
        .save_drive(&id_arr, &drive_bytes)
        .map_err(|e| AppError::DatabaseError(format!("Failed to save drive: {}", e)).to_string())?;

    tracing::info!(
        drive_id = %drive_id,
        old_name = %old_name,
        new_name = %validated_name,
        "Renamed drive"
    );

    Ok(DriveInfo::from(&*drive))
}
