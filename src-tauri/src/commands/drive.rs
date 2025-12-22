use crate::core::{file, DriveInfo, SharedDrive};
use crate::state::AppState;
use tauri::State;

/// Create a new shared drive from a local folder
#[tauri::command]
pub async fn create_drive(
    name: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<DriveInfo, String> {
    let local_path = std::path::PathBuf::from(&path);

    // Validate path exists and is a directory
    if !local_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    if !local_path.is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    // Get owner identity
    let owner = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| "Identity not initialized".to_string())?;

    // Create drive
    let mut drive = SharedDrive::new(name.clone(), local_path.clone(), owner);

    // Index files and update stats
    let entries = file::index_directory(&local_path)
        .map_err(|e| format!("Failed to index directory: {}", e))?;

    let total_size: u64 = entries.iter().map(|e| e.size).sum();
    let file_count = entries.iter().filter(|e| !e.is_dir).count() as u64;
    drive.update_stats(total_size, file_count);

    // Save to database
    let drive_bytes =
        serde_json::to_vec(&drive).map_err(|e| format!("Failed to serialize drive: {}", e))?;

    state
        .db
        .save_drive(drive.id.as_bytes(), &drive_bytes)
        .map_err(|e| format!("Failed to save drive: {}", e))?;

    // Add to in-memory cache
    state
        .drives
        .write()
        .await
        .insert(*drive.id.as_bytes(), drive.clone());

    tracing::info!(
        "Created drive '{}' at {} ({} files, {} bytes)",
        name,
        path,
        file_count,
        total_size
    );

    Ok(DriveInfo::from(&drive))
}

/// List all owned drives
#[tauri::command]
pub async fn list_drives(state: State<'_, AppState>) -> Result<Vec<DriveInfo>, String> {
    let drives = state.drives.read().await;
    let infos: Vec<DriveInfo> = drives.values().map(DriveInfo::from).collect();
    Ok(infos)
}

/// Get a specific drive by ID
#[tauri::command]
pub async fn get_drive(drive_id: String, state: State<'_, AppState>) -> Result<DriveInfo, String> {
    let id_bytes = hex::decode(&drive_id).map_err(|_| "Invalid drive ID format".to_string())?;

    if id_bytes.len() != 32 {
        return Err("Invalid drive ID length".to_string());
    }

    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&id_bytes);

    let drives = state.drives.read().await;
    let drive = drives
        .get(&id_arr)
        .ok_or_else(|| "Drive not found".to_string())?;

    Ok(DriveInfo::from(drive))
}
