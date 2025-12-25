//! File listing commands with path traversal protection
//!
//! All file operations validate paths to prevent directory traversal attacks.
//! All operations enforce ACL permission checks.
//! Supports optional E2E encryption via EncryptionManager.

use crate::commands::security::SecurityStore;
use crate::core::{file, validate_drive_id, validate_path, AppError, DriveId, FileEntryDto};
use crate::crypto::{EncryptionManager, Permission};
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

/// List files in a drive directory
///
/// Returns merged view of:
/// - Local files (is_local = true)
/// - Synced metadata from peers (is_local = false if not downloaded)
///
/// # Security
/// - Validates drive ID format
/// - Prevents directory traversal attacks
/// - Ensures path stays within drive root
/// - Enforces ACL permission checks (requires Read permission)
#[tauri::command]
pub async fn list_files(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<Vec<FileEntryDto>, String> {
    // Validate drive ID
    let id_arr = validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    let drive_id_obj = DriveId(id_arr);

    // Get drive
    let drives = state.drives.read().await;
    let drive = drives.get(&id_arr).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;
    let local_path = drive.local_path.clone();
    let owner_hex = drive.owner.to_hex();
    drop(drives);

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();

    // Enforce ACL permission check
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &path, Permission::Read) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to list files"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to list files".to_string(),
        }
        .to_string());
    }

    // Collect files into a map keyed by path for merging
    let mut files_map: HashMap<String, FileEntryDto> = HashMap::new();

    // 1. First, get synced metadata from DocsManager (remote files)
    if let Some(docs_manager) = state.docs_manager.as_ref() {
        match docs_manager.get_directory_metadata(&drive_id_obj, &path).await {
            Ok(metadata) => {
                for meta in metadata {
                    let dto = FileEntryDto::from_metadata(
                        meta.name.clone(),
                        meta.path.clone(),
                        meta.is_dir,
                        meta.size,
                        meta.modified_at.clone(),
                        meta.content_hash.clone(),
                    );
                    files_map.insert(meta.path.clone(), dto);
                }
                tracing::debug!(
                    drive_id = %drive_id,
                    path = %path,
                    synced_count = files_map.len(),
                    "Loaded synced metadata"
                );
            }
            Err(e) => {
                tracing::debug!(
                    drive_id = %drive_id,
                    error = %e,
                    "No synced metadata available (this is normal for new drives)"
                );
            }
        }
    }

    // 2. Then, get local files from filesystem and merge (override remote entries)
    // Validate path is safe (prevents directory traversal)
    let safe_path = validate_path(&local_path, &path).map_err(|e| e.to_string())?;

    // Check if local directory exists
    if safe_path.exists() && safe_path.is_dir() {
        match file::list_directory(&local_path, &path) {
            Ok(entries) => {
                for entry in entries {
                    let entry_path = entry.path.to_string_lossy().to_string();
                    let mut dto = FileEntryDto::from(&entry);
                    
                    // If we have synced metadata for this file, copy the content_hash
                    if let Some(synced) = files_map.get(&entry_path) {
                        dto.content_hash = synced.content_hash.clone();
                    }
                    
                    // Local file - is_local is already true from From impl
                    files_map.insert(entry_path, dto);
                }
            }
            Err(e) => {
                tracing::warn!(
                    drive_id = %drive_id,
                    path = %path,
                    error = %e,
                    "Failed to list local directory"
                );
            }
        }
    }

    // Convert map to sorted vector
    let mut dtos: Vec<FileEntryDto> = files_map.into_values().collect();
    
    // Sort: directories first, then by name (case-insensitive)
    dtos.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    tracing::debug!(
        drive_id = %drive_id,
        path = %path,
        total_count = dtos.len(),
        local_count = dtos.iter().filter(|f| f.is_local).count(),
        remote_count = dtos.iter().filter(|f| !f.is_local).count(),
        "Listed files (merged local + synced)"
    );

    Ok(dtos)
}

/// File content response
#[derive(Clone, Debug, serde::Serialize)]
pub struct FileContent {
    /// Base64 encoded file content
    pub content: String,
    /// File size in bytes
    pub size: u64,
    /// Detected MIME type (optional)
    pub mime_type: Option<String>,
}

/// Read file content from a drive
///
/// # Security
/// - Validates drive ID format
/// - Prevents directory traversal attacks
/// - Ensures path stays within drive root
/// - Enforces ACL permission checks (requires Read permission)
#[tauri::command]
pub async fn read_file(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<FileContent, String> {
    use base64::Engine;

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

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();
    let owner_hex = drive.owner.to_hex();

    // Enforce ACL permission check
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &path, Permission::Read) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to read file"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to read file".to_string(),
        }
        .to_string());
    }

    // Validate path is safe (prevents directory traversal)
    let safe_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;

    // Ensure the path exists
    if !safe_path.exists() {
        return Err(AppError::PathNotFound { path: path.clone() }.to_string());
    }

    // Ensure it's a file, not a directory
    if safe_path.is_dir() {
        return Err(AppError::NotAFile { path }.to_string());
    }

    // Read file content
    let content = std::fs::read(&safe_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let size = content.len() as u64;

    // Detect MIME type from extension
    let mime_type = safe_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "txt" | "md" | "rs" | "js" | "ts" | "py" | "json" | "toml" | "yaml" | "yml" => {
                "text/plain"
            }
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        })
        .map(String::from);

    // Encode content as base64
    let encoded = base64::engine::general_purpose::STANDARD.encode(&content);

    tracing::debug!(
        drive_id = %drive_id,
        path = %path,
        size = size,
        "Read file content"
    );

    Ok(FileContent {
        content: encoded,
        size,
        mime_type,
    })
}

/// Write content to a file in a drive
///
/// # Security
/// - Validates drive ID format
/// - Prevents directory traversal attacks
/// - Ensures path stays within drive root
/// - Creates parent directories if needed
/// - Enforces ACL permission checks (requires Write permission)
#[tauri::command]
pub async fn write_file(
    drive_id: String,
    path: String,
    content: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<(), String> {
    use base64::Engine;

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

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();
    let owner_hex = drive.owner.to_hex();

    // Enforce ACL permission check (requires Write)
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &path, Permission::Write) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to write file"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to write file".to_string(),
        }
        .to_string());
    }

    // Validate path is safe (prevents directory traversal)
    let safe_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;

    // Ensure it's not trying to overwrite the drive root
    if safe_path == drive.local_path {
        return Err("Cannot write to drive root".to_string());
    }

    // Decode base64 content
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&content)
        .map_err(|e| format!("Invalid base64 content: {}", e))?;

    // Create parent directories if needed
    if let Some(parent) = safe_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    // Write file content
    std::fs::write(&safe_path, &decoded).map_err(|e| format!("Failed to write file: {}", e))?;

    tracing::info!(
        drive_id = %drive_id,
        path = %path,
        size = decoded.len(),
        "Wrote file content"
    );

    Ok(())
}

/// Delete a file or directory from a drive
///
/// # Security
/// - Validates drive ID format
/// - Prevents directory traversal attacks
/// - Ensures path stays within drive root
/// - Enforces ACL permission checks (requires Write permission)
#[tauri::command]
pub async fn delete_path(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<(), String> {
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

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();
    let owner_hex = drive.owner.to_hex();

    // Enforce ACL permission check (requires Write to delete)
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &path, Permission::Write) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to delete path"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to delete path".to_string(),
        }
        .to_string());
    }

    // Validate path is safe
    let safe_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;

    // Ensure the path exists
    if !safe_path.exists() {
        return Err(AppError::PathNotFound { path: path.clone() }.to_string());
    }

    // Don't allow deleting the drive root
    if safe_path == drive.local_path {
        return Err("Cannot delete drive root".to_string());
    }

    // Delete file or directory
    if safe_path.is_dir() {
        std::fs::remove_dir_all(&safe_path)
            .map_err(|e| format!("Failed to delete directory: {}", e))?;
    } else {
        std::fs::remove_file(&safe_path).map_err(|e| format!("Failed to delete file: {}", e))?;
    }

    tracing::info!(
        drive_id = %drive_id,
        path = %path,
        "Deleted path"
    );

    Ok(())
}

/// Rename/move a file or directory within a drive
///
/// # Security
/// - Validates drive ID format
/// - Prevents directory traversal attacks
/// - Ensures both paths stay within drive root
/// - Enforces ACL permission checks (requires Write permission on both paths)
#[tauri::command]
pub async fn rename_path(
    drive_id: String,
    old_path: String,
    new_path: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<(), String> {
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

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();
    let owner_hex = drive.owner.to_hex();

    // Enforce ACL permission check (requires Write on both old and new paths)
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &old_path, Permission::Write) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %old_path,
            "Access denied: insufficient permission to rename from source path"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to rename from source path".to_string(),
        }
        .to_string());
    }
    if !acl.check_permission(&caller_hex, &new_path, Permission::Write) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %new_path,
            "Access denied: insufficient permission to rename to destination path"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to rename to destination path".to_string(),
        }
        .to_string());
    }

    // Validate both paths are safe
    let safe_old = validate_path(&drive.local_path, &old_path).map_err(|e| e.to_string())?;
    let safe_new = validate_path(&drive.local_path, &new_path).map_err(|e| e.to_string())?;

    // Ensure old path exists
    if !safe_old.exists() {
        return Err(AppError::PathNotFound {
            path: old_path.clone(),
        }
        .to_string());
    }

    // Don't allow renaming drive root
    if safe_old == drive.local_path {
        return Err("Cannot rename drive root".to_string());
    }

    // Create parent directories for new path if needed
    if let Some(parent) = safe_new.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    // Rename/move
    std::fs::rename(&safe_old, &safe_new).map_err(|e| format!("Failed to rename: {}", e))?;

    tracing::info!(
        drive_id = %drive_id,
        old_path = %old_path,
        new_path = %new_path,
        "Renamed path"
    );

    Ok(())
}

/// Read encrypted file content from a drive
///
/// # Security
/// - Same validations as read_file
/// - Decrypts content using the drive's encryption key
#[tauri::command]
pub async fn read_file_encrypted(
    drive_id: String,
    path: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
    encryption: State<'_, Arc<EncryptionManager>>,
) -> Result<FileContent, String> {
    use base64::Engine;

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

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();
    let owner_hex = drive.owner.to_hex();

    // Enforce ACL permission check
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &path, Permission::Read) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to read file"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to read file".to_string(),
        }
        .to_string());
    }

    // Validate path is safe
    let safe_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;

    // Ensure the path exists and is a file
    if !safe_path.exists() {
        return Err(AppError::PathNotFound { path: path.clone() }.to_string());
    }
    if safe_path.is_dir() {
        return Err(AppError::NotAFile { path: path.clone() }.to_string());
    }

    // Read encrypted file content
    let encrypted_content = std::fs::read(&safe_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Decrypt the content
    let content = encryption
        .decrypt_file(&drive_id, &path, &encrypted_content)
        .await
        .map_err(|e| format!("Decryption failed: {}", e))?;

    let size = content.len() as u64;

    // Detect MIME type from extension
    let mime_type = safe_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "txt" | "md" | "rs" | "js" | "ts" | "py" | "json" | "toml" | "yaml" | "yml" => {
                "text/plain"
            }
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        })
        .map(String::from);

    // Encode decrypted content as base64
    let encoded = base64::engine::general_purpose::STANDARD.encode(&content);

    tracing::debug!(
        drive_id = %drive_id,
        path = %path,
        size = size,
        "Read encrypted file content"
    );

    Ok(FileContent {
        content: encoded,
        size,
        mime_type,
    })
}

/// Write encrypted content to a file in a drive
///
/// # Security
/// - Same validations as write_file
/// - Encrypts content using the drive's encryption key
#[tauri::command]
pub async fn write_file_encrypted(
    drive_id: String,
    path: String,
    content: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
    encryption: State<'_, Arc<EncryptionManager>>,
) -> Result<(), String> {
    use base64::Engine;

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

    // Get caller identity and check permission
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;
    let caller_hex = caller.to_hex();
    let owner_hex = drive.owner.to_hex();

    // Enforce ACL permission check (requires Write)
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;
    if !acl.check_permission(&caller_hex, &path, Permission::Write) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            path = %path,
            "Access denied: insufficient permission to write file"
        );
        return Err(AppError::AccessDenied {
            reason: "insufficient permission to write file".to_string(),
        }
        .to_string());
    }

    // Validate path is safe
    let safe_path = validate_path(&drive.local_path, &path).map_err(|e| e.to_string())?;

    if safe_path == drive.local_path {
        return Err("Cannot write to drive root".to_string());
    }

    // Decode base64 content
    let plaintext = base64::engine::general_purpose::STANDARD
        .decode(&content)
        .map_err(|e| format!("Invalid base64 content: {}", e))?;

    // Encrypt the content
    let encrypted_content = encryption
        .encrypt_file(&drive_id, &path, &plaintext)
        .await
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Create parent directories if needed
    if let Some(parent) = safe_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    // Write encrypted content
    std::fs::write(&safe_path, &encrypted_content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    tracing::info!(
        drive_id = %drive_id,
        path = %path,
        size = plaintext.len(),
        encrypted_size = encrypted_content.len(),
        "Wrote encrypted file content"
    );

    Ok(())
}
