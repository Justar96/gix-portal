use crate::core::{file, FileEntryDto};
use crate::state::AppState;
use tauri::State;

/// List files in a drive directory
#[tauri::command]
pub async fn list_files(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntryDto>, String> {
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

    let entries = file::list_directory(&drive.local_path, &path)
        .map_err(|e| format!("Failed to list directory: {}", e))?;

    let dtos: Vec<FileEntryDto> = entries.iter().map(FileEntryDto::from).collect();
    Ok(dtos)
}
