//! Security validation utilities for production safety
//!
//! Provides input validation, path sanitization, and security checks
//! to prevent common vulnerabilities.

use crate::core::error::AppError;
use std::path::{Path, PathBuf};

/// Maximum allowed name length for drives and other entities
pub const MAX_NAME_LENGTH: usize = 255;

/// Minimum name length
pub const MIN_NAME_LENGTH: usize = 1;

/// Maximum path depth to prevent resource exhaustion
pub const MAX_PATH_DEPTH: usize = 64;

/// Characters forbidden in names (for cross-platform compatibility)
const FORBIDDEN_NAME_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\0'];

/// Patterns that indicate path traversal attempts
const TRAVERSAL_PATTERNS: &[&str] = &[
    "..",
    "..\\",
    "../",
    "...\\",
    ".../",
    "%2e%2e",    // URL encoded ..
    "%2e%2e%2f", // URL encoded ../
    "%2e%2e/",
    "..%2f",
    "%2e%2e%5c", // URL encoded ..\
    "..%5c",
    "%252e", // Double URL encoded
];

/// Validate and sanitize a path to prevent directory traversal attacks
///
/// # Arguments
/// * `base_path` - The root directory that the path must stay within
/// * `user_path` - The user-provided path to validate
///
/// # Returns
/// * `Ok(PathBuf)` - The canonicalized safe path
/// * `Err(AppError)` - If the path is invalid or attempts traversal
pub fn validate_path(base_path: &Path, user_path: &str) -> Result<PathBuf, AppError> {
    // Check for empty path
    if user_path.is_empty() {
        return Ok(base_path.to_path_buf());
    }

    // Check for obvious traversal patterns
    let normalized = user_path.replace('\\', "/");
    for pattern in TRAVERSAL_PATTERNS {
        if normalized.contains(pattern) {
            tracing::warn!(
                path = %user_path,
                pattern = %pattern,
                "Path traversal attempt detected"
            );
            return Err(AppError::PathTraversal {
                path: user_path.to_string(),
            });
        }
    }

    // Check path depth
    let depth = normalized.split('/').filter(|s| !s.is_empty()).count();
    if depth > MAX_PATH_DEPTH {
        return Err(AppError::InvalidPath {
            path: user_path.to_string(),
            reason: format!("Path too deep (max {} levels)", MAX_PATH_DEPTH),
        });
    }

    // Construct the full path
    let full_path = if user_path.starts_with('/') || user_path.starts_with('\\') {
        base_path.join(user_path.trim_start_matches(['/', '\\']))
    } else {
        base_path.join(user_path)
    };

    // Canonicalize both paths for comparison
    // Note: canonicalize requires the path to exist, so we use a different approach
    let resolved = normalize_path(&full_path);
    let base_resolved = normalize_path(base_path);

    // Ensure the resolved path is within the base path
    if !resolved.starts_with(&base_resolved) {
        tracing::warn!(
            base = %base_path.display(),
            path = %user_path,
            resolved = %resolved.display(),
            "Path escapes root directory"
        );
        return Err(AppError::PathOutsideDrive {
            path: user_path.to_string(),
        });
    }

    Ok(resolved)
}

/// Normalize a path without requiring it to exist
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Only pop if we have something to pop and it's not root
                if normalized.parent().is_some() && normalized != Path::new("/") {
                    normalized.pop();
                }
            }
            std::path::Component::CurDir => {
                // Skip current dir references
            }
            _ => {
                normalized.push(component);
            }
        }
    }

    normalized
}

/// Validate a name (drive name, file name, etc.)
///
/// # Arguments
/// * `name` - The name to validate
/// * `field_name` - The field name for error messages
///
/// # Returns
/// * `Ok(String)` - The trimmed, valid name
/// * `Err(AppError)` - If the name is invalid
pub fn validate_name(name: &str, field_name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();

    // Check minimum length
    if trimmed.len() < MIN_NAME_LENGTH {
        return Err(AppError::NameEmpty);
    }

    // Check length
    if trimmed.len() > MAX_NAME_LENGTH {
        return Err(AppError::NameTooLong {
            max: MAX_NAME_LENGTH,
        });
    }

    // Check for forbidden characters
    for ch in FORBIDDEN_NAME_CHARS {
        if trimmed.contains(*ch) {
            return Err(AppError::ValidationFailed {
                field: field_name.to_string(),
                reason: format!("Contains forbidden character: '{}'", ch),
            });
        }
    }

    // Check for control characters
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(AppError::ValidationFailed {
            field: field_name.to_string(),
            reason: "Contains control characters".to_string(),
        });
    }

    if !is_safe_filename(trimmed) {
        return Err(AppError::ValidationFailed {
            field: field_name.to_string(),
            reason: "Reserved filename".to_string(),
        });
    }

    // Check for leading/trailing dots or spaces (Windows compatibility)
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        // Allow this for hidden files on Unix, but warn
        tracing::debug!(name = %trimmed, "Name starts or ends with dot");
    }

    Ok(trimmed.to_string())
}

/// Validate a hex-encoded drive ID
///
/// # Arguments
/// * `drive_id` - The hex string to validate
///
/// # Returns
/// * `Ok([u8; 32])` - The decoded drive ID bytes
/// * `Err(AppError)` - If the ID is invalid
pub fn validate_drive_id(drive_id: &str) -> Result<[u8; 32], AppError> {
    // Check length first (64 hex chars = 32 bytes)
    if drive_id.len() != 64 {
        return Err(AppError::InvalidDriveId {
            id: format!("Expected 64 hex chars, got {}", drive_id.len()),
        });
    }

    // Decode hex
    let bytes = hex::decode(drive_id).map_err(|_| AppError::InvalidDriveId {
        id: "Invalid hex encoding".to_string(),
    })?;

    // Convert to fixed array
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Validate a node ID (public key hex)
pub fn validate_node_id(node_id: &str) -> Result<[u8; 32], AppError> {
    if node_id.len() != 64 {
        return Err(AppError::ValidationFailed {
            field: "node_id".to_string(),
            reason: format!("Expected 64 hex chars, got {}", node_id.len()),
        });
    }

    let bytes = hex::decode(node_id).map_err(|_| AppError::ValidationFailed {
        field: "node_id".to_string(),
        reason: "Invalid hex encoding".to_string(),
    })?;

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Check if a file path is safe for filesystem operations
pub fn is_safe_filename(name: &str) -> bool {
    // Empty names are not safe
    if name.is_empty() {
        return false;
    }

    // Check for forbidden chars
    for ch in FORBIDDEN_NAME_CHARS {
        if name.contains(*ch) {
            return false;
        }
    }

    // Check for reserved Windows names
    let upper = name.to_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let name_without_ext = upper.split('.').next().unwrap_or(&upper);
    if reserved.contains(&name_without_ext) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_validate_path_normal() {
        let base = Path::new("/home/user/drive");
        let result = validate_path(base, "documents/file.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        let base = Path::new("/home/user/drive");

        // Test various traversal attempts
        let attacks = vec![
            "../etc/passwd",
            "..\\windows\\system32",
            "documents/../../../etc/passwd",
            "documents%2f..%2f..%2fetc/passwd",
            "...\\secret",
        ];

        for attack in attacks {
            let result = validate_path(base, attack);
            assert!(result.is_err(), "Should reject: {}", attack);
        }
    }

    #[test]
    fn test_validate_path_absolute() {
        let base = Path::new("/home/user/drive");
        let result = validate_path(base, "/subdir/file.txt");
        assert!(result.is_ok());
        // Should be within base
        let path = result.unwrap();
        assert!(path.starts_with(base));
    }

    #[test]
    fn test_validate_name_empty() {
        let result = validate_name("", "test");
        assert!(matches!(result, Err(AppError::NameEmpty)));
    }

    #[test]
    fn test_validate_name_too_long() {
        let long_name = "a".repeat(300);
        let result = validate_name(&long_name, "test");
        assert!(matches!(result, Err(AppError::NameTooLong { .. })));
    }

    #[test]
    fn test_validate_name_forbidden_chars() {
        let result = validate_name("file<name", "test");
        assert!(result.is_err());

        let result = validate_name("file|name", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_name_reserved() {
        let result = validate_name("CON", "test");
        assert!(result.is_err());

        let result = validate_name("COM1.txt", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_name_valid() {
        let result = validate_name("  My Drive Name  ", "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "My Drive Name");
    }

    #[test]
    fn test_validate_drive_id_valid() {
        let id = "a".repeat(64);
        let result = validate_drive_id(&id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_drive_id_invalid() {
        // Wrong length
        let result = validate_drive_id("abc");
        assert!(result.is_err());

        // Invalid hex
        let result = validate_drive_id(&"g".repeat(64));
        assert!(result.is_err());
    }

    #[test]
    fn test_is_safe_filename() {
        assert!(is_safe_filename("normal_file.txt"));
        assert!(!is_safe_filename("file<name.txt"));
        assert!(!is_safe_filename("CON"));
        assert!(!is_safe_filename("COM1.txt"));
        assert!(!is_safe_filename(""));
    }
}
