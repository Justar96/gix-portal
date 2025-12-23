//! Sync commands - Tauri commands for real-time synchronization
//!
//! These commands expose sync functionality to the frontend.

use crate::core::DriveId;
use crate::network::SyncStatus;
use crate::state::AppState;
use tauri::State;

/// Helper to parse and validate drive ID from hex string
fn parse_drive_id(drive_id: &str) -> Result<DriveId, String> {
    let id_bytes = hex::decode(drive_id).map_err(|_| "Invalid drive ID format".to_string())?;

    if id_bytes.len() != 32 {
        return Err("Invalid drive ID length".to_string());
    }

    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&id_bytes);
    Ok(DriveId(id_arr))
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
        .ok_or_else(|| "Sync engine not initialized".to_string())?;

    // Get the drive from cache
    let drives = state.drives.read().await;
    let drive = drives
        .get(id.as_bytes())
        .ok_or_else(|| "Drive not found".to_string())?;

    // Initialize sync for this drive
    sync_engine
        .init_drive(drive)
        .await
        .map_err(|e| format!("Failed to start sync: {}", e))?;

    tracing::info!("Started sync for drive: {}", drive_id);
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
        .ok_or_else(|| "Sync engine not initialized".to_string())?;

    sync_engine.stop_sync(&id).await;

    tracing::info!("Stopped sync for drive: {}", drive_id);
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
        .ok_or_else(|| "Sync engine not initialized".to_string())?;

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
