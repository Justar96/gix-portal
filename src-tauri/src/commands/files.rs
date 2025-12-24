//! File listing commands with path traversal protection
//!
//! All file operations validate paths to prevent directory traversal attacks.

use crate::core::{file, validate_drive_id, validate_path, AppError, FileEntryDto};
use crate::state::AppState;
use tauri::State;

/// List files in a drive directory
///
/// # Security
/// - Validates drive ID format
/// - Prevents directory traversal attacks
/// - Ensures path stays within drive root
#[tauri::command]
pub async fn list_files(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntryDto>, String> {
    // Validate drive ID
    let id_arr = validate_drive_id(&drive_id).map_err(|e| e.to_string())?;

    // Get drive
    let drives = state.drives.read().await;
    let drive = drives.get(&id_arr).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    // Validate path is safe (prevents directory traversal)
    let safe_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;

    // Ensure the path exists and is within drive
    if !safe_path.exists() {
        return Err(AppError::PathNotFound {
            path: path.clone(),
        }
        .to_string());
    }

    if !safe_path.is_dir() {
        return Err(AppError::NotADirectory { path }.to_string());
    }

    // List directory contents
    let entries = file::list_directory(&drive.local_path, &path)
        .map_err(|e| format!("Failed to list directory: {}", e))?;

    let dtos: Vec<FileEntryDto> = entries.iter().map(FileEntryDto::from).collect();

    tracing::debug!(
        drive_id = %drive_id,
        path = %path,
        count = dtos.len(),
        "Listed files"
    );

    Ok(dtos)
}
