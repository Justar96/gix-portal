//! Audit log commands for security event tracking
//!
//! Provides Tauri commands to query and manage audit logs for
//! security monitoring and compliance.

use crate::core::{AuditEntryDto, AuditFilter, AuditLogger};
use std::sync::Arc;
use tauri::State;

/// Get audit log entries with optional filters
///
/// # Arguments
/// * `drive_id` - Optional filter by drive ID
/// * `event_type` - Optional filter by event type
/// * `user_id` - Optional filter by user ID
/// * `since` - Optional filter for events after this timestamp (Unix ms)
/// * `limit` - Maximum number of entries to return (default: 100)
/// * `offset` - Number of entries to skip (for pagination)
#[tauri::command]
pub async fn get_audit_log(
    drive_id: Option<String>,
    event_type: Option<String>,
    user_id: Option<String>,
    since: Option<i64>,
    limit: Option<usize>,
    offset: Option<usize>,
    audit_logger: State<'_, Arc<AuditLogger>>,
) -> Result<Vec<AuditEntryDto>, String> {
    let filter = AuditFilter {
        drive_id,
        event_type,
        user_id,
        since,
        until: None,
        limit,
        offset,
    };

    let entries = audit_logger
        .query(filter)
        .await
        .map_err(|e| format!("Failed to query audit log: {}", e))?;

    Ok(entries.into_iter().map(AuditEntryDto::from).collect())
}

/// Get the total count of audit log entries
#[tauri::command]
pub async fn get_audit_count(
    audit_logger: State<'_, Arc<AuditLogger>>,
) -> Result<u64, String> {
    audit_logger
        .count()
        .await
        .map_err(|e| format!("Failed to count audit entries: {}", e))
}

/// Get recent security events for a specific drive
#[tauri::command]
pub async fn get_drive_audit_log(
    drive_id: String,
    limit: Option<usize>,
    audit_logger: State<'_, Arc<AuditLogger>>,
) -> Result<Vec<AuditEntryDto>, String> {
    let entries = audit_logger
        .get_drive_events(&drive_id, limit.unwrap_or(50))
        .await
        .map_err(|e| format!("Failed to get drive audit log: {}", e))?;

    Ok(entries.into_iter().map(AuditEntryDto::from).collect())
}

/// Get access denied events for security monitoring
#[tauri::command]
pub async fn get_denied_access_log(
    limit: Option<usize>,
    audit_logger: State<'_, Arc<AuditLogger>>,
) -> Result<Vec<AuditEntryDto>, String> {
    let entries = audit_logger
        .get_denied_access_events(limit.unwrap_or(100))
        .await
        .map_err(|e| format!("Failed to get denied access log: {}", e))?;

    Ok(entries.into_iter().map(AuditEntryDto::from).collect())
}
