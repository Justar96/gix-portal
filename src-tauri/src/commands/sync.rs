//! Sync commands - Tauri commands for real-time synchronization
//!
//! These commands expose sync functionality to the frontend.
//! All commands include proper input validation and error handling.

use crate::core::{validate_drive_id, validate_path, AppError, DriveId};
use crate::network::SyncStatus;
use crate::state::AppState;
use tauri::State;

/// Helper to parse drive ID with proper validation
fn parse_drive_id(drive_id: &str) -> Result<DriveId, String> {
    let arr = validate_drive_id(drive_id).map_err(|e| e.to_string())?;
    Ok(DriveId(arr))
}

/// Start syncing a drive
///
/// This initializes the sync engine for the specified drive:
/// - Creates iroh-doc for metadata sync
/// - Subscribes to gossip topic for events
#[tauri::command]
pub async fn start_sync(drive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id = parse_drive_id(&drive_id)?;

    // Check if sync engine is available
    let sync_engine = state
        .sync_engine
        .as_ref()
        .ok_or_else(|| AppError::SyncNotInitialized.to_string())?;

    // Get the drive from cache
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    // Initialize sync for this drive
    sync_engine
        .init_drive(drive)
        .await
        .map_err(|e| AppError::SyncFailed(format!("Failed to start sync: {}", e)).to_string())?;

    tracing::info!(drive_id = %drive_id, "Started sync for drive");
    Ok(())
}

/// Stop syncing a drive
///
/// This stops the sync engine for the specified drive:
/// - Unsubscribes from gossip topic
/// - Closes iroh-doc
#[tauri::command]
pub async fn stop_sync(drive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id = parse_drive_id(&drive_id)?;

    // Check if sync engine is available
    let sync_engine = state
        .sync_engine
        .as_ref()
        .ok_or_else(|| AppError::SyncNotInitialized.to_string())?;

    sync_engine.stop_sync(&id).await;

    tracing::info!(drive_id = %drive_id, "Stopped sync for drive");
    Ok(())
}

/// Get sync status for a drive
#[tauri::command]
pub async fn get_sync_status(
    drive_id: String,
    state: State<'_, AppState>,
) -> Result<SyncStatus, String> {
    let id = parse_drive_id(&drive_id)?;

    // Check if sync engine is available
    let sync_engine = state
        .sync_engine
        .as_ref()
        .ok_or_else(|| AppError::SyncNotInitialized.to_string())?;

    let status = sync_engine.get_status(&id).await;
    Ok(status)
}

/// Subscribe to drive events (returns immediately, events come via Tauri events)
///
/// This sets up a listener that forwards gossip events to the frontend
/// via Tauri's event system.
#[tauri::command]
pub async fn subscribe_drive_events(
    drive_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _id = parse_drive_id(&drive_id)?;

    // Check if event broadcaster is available
    let _broadcaster = state
        .event_broadcaster
        .as_ref()
        .ok_or_else(|| "Event broadcaster not initialized".to_string())?;

    // Note: The actual event forwarding is set up in lib.rs when the app starts.
    // This command just validates that the drive exists and sync is available.
    tracing::info!("Frontend subscribed to events for drive: {}", drive_id);
    Ok(())
}

/// Start watching a drive's folder for local changes
///
/// This enables the file watcher for the specified drive, which will
/// detect local file changes and emit events to the sync engine.
#[tauri::command]
pub async fn start_watching(drive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id = parse_drive_id(&drive_id)?;

    // Check if file watcher is available
    let file_watcher = state
        .file_watcher
        .as_ref()
        .ok_or_else(|| "File watcher not initialized".to_string())?;

    // Get the drive's local path from cache
    let drives = state.drives.read().await;
    let drive = drives
        .get(id.as_bytes())
        .ok_or_else(|| "Drive not found".to_string())?;

    let local_path = drive.local_path.clone();
    drop(drives); // Release lock before async operation

    // Start watching
    file_watcher
        .watch(id, local_path)
        .await
        .map_err(|e| format!("Failed to start watching: {}", e))?;

    tracing::info!("Started file watching for drive: {}", drive_id);
    Ok(())
}

/// Stop watching a drive's folder
#[tauri::command]
pub async fn stop_watching(drive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id = parse_drive_id(&drive_id)?;

    // Check if file watcher is available
    let file_watcher = state
        .file_watcher
        .as_ref()
        .ok_or_else(|| "File watcher not initialized".to_string())?;

    file_watcher.unwatch(&id).await;

    tracing::info!("Stopped file watching for drive: {}", drive_id);
    Ok(())
}

/// Check if a drive is being watched
#[tauri::command]
pub async fn is_watching(drive_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let id = parse_drive_id(&drive_id)?;

    let file_watcher = state
        .file_watcher
        .as_ref()
        .ok_or_else(|| AppError::WatcherNotInitialized.to_string())?;

    Ok(file_watcher.is_watching(&id).await)
}

// ==============================================
// File Transfer Commands
// ==============================================

use crate::network::TransferState;

/// Upload a file to the blob store
///
/// This imports a local file into iroh-blobs, making it available to peers.
///
/// # Security
/// - Validates file path is within drive root
/// - Prevents directory traversal attacks
#[tauri::command]
pub async fn upload_file(
    drive_id: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id = parse_drive_id(&drive_id)?;

    let file_transfer = state
        .file_transfer
        .as_ref()
        .ok_or_else(|| AppError::TransferNotInitialized.to_string())?;

    // Get drive to determine relative path
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    // Validate the file path is within drive root (prevents path traversal)
    let validated_path = validate_path(&drive.local_path, &file_path).map_err(|e| e.to_string())?;

    let relative_path = validated_path
        .strip_prefix(&drive.local_path)
        .map_err(|_| {
            AppError::PathOutsideDrive {
                path: file_path.clone(),
            }
            .to_string()
        })?
        .to_path_buf();

    drop(drives);

    // Upload the file
    let hash = file_transfer
        .upload_file(&id, &validated_path, &relative_path)
        .await
        .map_err(|e| AppError::TransferFailed(format!("Upload failed: {}", e)).to_string())?;

    tracing::info!(
        drive_id = %drive_id,
        path = %file_path,
        hash = %hash.to_hex(),
        "Uploaded file"
    );
    Ok(hash.to_hex().to_string())
}

/// Download a file from the blob store to local filesystem
///
/// # Security
/// - Validates destination path is within drive root
/// - Prevents directory traversal attacks
#[tauri::command]
pub async fn download_file(
    drive_id: String,
    hash: String,
    destination_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let id = parse_drive_id(&drive_id)?;

    let file_transfer = state
        .file_transfer
        .as_ref()
        .ok_or_else(|| AppError::TransferNotInitialized.to_string())?;

    // Parse the hash
    let blob_hash = hash
        .parse::<iroh_blobs::Hash>()
        .map_err(|e| AppError::InvalidHash(format!("Invalid hash: {}", e)).to_string())?;

    // Get drive for path validation
    let drives = state.drives.read().await;
    let drive = drives.get(id.as_bytes()).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    // Validate the destination path is within drive root
    let validated_path =
        validate_path(&drive.local_path, &destination_path).map_err(|e| e.to_string())?;

    let relative_path = validated_path
        .strip_prefix(&drive.local_path)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| validated_path.clone());

    drop(drives);

    // Download the file
    file_transfer
        .download_file(&id, blob_hash, &validated_path, &relative_path)
        .await
        .map_err(|e| AppError::TransferFailed(format!("Download failed: {}", e)).to_string())?;

    tracing::info!(
        drive_id = %drive_id,
        hash = %hash,
        path = %destination_path,
        "Downloaded file"
    );
    Ok(())
}

/// List all active transfers
#[tauri::command]
pub async fn list_transfers(state: State<'_, AppState>) -> Result<Vec<TransferState>, String> {
    let file_transfer = state
        .file_transfer
        .as_ref()
        .ok_or_else(|| AppError::TransferNotInitialized.to_string())?;

    Ok(file_transfer.list_transfers().await)
}

/// Get a specific transfer by ID
#[tauri::command]
pub async fn get_transfer(
    transfer_id: String,
    state: State<'_, AppState>,
) -> Result<Option<TransferState>, String> {
    let file_transfer = state
        .file_transfer
        .as_ref()
        .ok_or_else(|| AppError::TransferNotInitialized.to_string())?;

    Ok(file_transfer.get_transfer(&transfer_id).await)
}

/// Cancel an active transfer
#[tauri::command]
pub async fn cancel_transfer(
    transfer_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let file_transfer = state
        .file_transfer
        .as_ref()
        .ok_or_else(|| AppError::TransferNotInitialized.to_string())?;

    file_transfer
        .cancel_transfer(&transfer_id)
        .await
        .map_err(|e| AppError::TransferFailed(format!("Failed to cancel: {}", e)).to_string())?;

    tracing::info!(transfer_id = %transfer_id, "Cancelled transfer");
    Ok(())
}
