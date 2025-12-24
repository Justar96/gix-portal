//! Tauri commands for presence and activity tracking
//!
//! Provides commands for querying online users and activity feed.
//! 
//! # Security
//! - Validates drive IDs before all operations
//! - Limits activity query results to prevent memory exhaustion

use crate::core::validation::validate_drive_id;
use crate::core::{ActivityEntryDto, PresenceManager, UserPresenceDto};
use std::sync::Arc;
use tauri::State;

/// Maximum number of activity entries to return
const MAX_ACTIVITY_LIMIT: usize = 500;

/// Get online users for a drive
#[tauri::command]
pub async fn get_online_users(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<Vec<UserPresenceDto>, String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    let users = presence_manager.get_online_users(&drive_id).await;
    let node_id = presence_manager.node_id();

    Ok(users
        .iter()
        .map(|u| UserPresenceDto::from_presence(u, node_id))
        .collect())
}

/// Get online user count for a drive
#[tauri::command]
pub async fn get_online_count(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<usize, String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    let manager = presence_manager.get_drive_presence(&drive_id).await;
    Ok(manager.online_count().await)
}

/// Get recent activity for a drive
#[tauri::command]
pub async fn get_recent_activity(
    drive_id: String,
    limit: Option<usize>,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<Vec<ActivityEntryDto>, String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    // Clamp limit to prevent memory exhaustion
    let limit = limit.unwrap_or(50).min(MAX_ACTIVITY_LIMIT);
    
    let activities = presence_manager.get_recent_activity(&drive_id, limit).await;
    let node_id = presence_manager.node_id();

    Ok(activities
        .iter()
        .map(|a| ActivityEntryDto::from_entry(a, node_id))
        .collect())
}

/// Join a drive (announce presence)
#[tauri::command]
pub async fn join_drive_presence(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<(), String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    presence_manager.join_drive(&drive_id).await;
    tracing::debug!(drive_id = %drive_id, "Joined drive presence");
    Ok(())
}

/// Leave a drive (remove presence)
#[tauri::command]
pub async fn leave_drive_presence(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<(), String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    presence_manager.leave_drive(&drive_id).await;
    tracing::debug!(drive_id = %drive_id, "Left drive presence");
    Ok(())
}

/// Send a heartbeat to keep presence alive
#[tauri::command]
pub async fn presence_heartbeat(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<(), String> {
    // Validate drive_id format
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;
    
    let manager = presence_manager.get_drive_presence(&drive_id).await;
    let node_id = *presence_manager.node_id();
    manager.user_heartbeat(node_id).await;
    Ok(())
}
