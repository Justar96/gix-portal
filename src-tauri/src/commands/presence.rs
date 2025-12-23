//! Tauri commands for presence and activity tracking
//!
//! Provides commands for querying online users and activity feed.

use crate::core::{ActivityEntryDto, PresenceManager, UserPresenceDto};
use std::sync::Arc;
use tauri::State;

/// Get online users for a drive
#[tauri::command]
pub async fn get_online_users(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<Vec<UserPresenceDto>, String> {
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
    let limit = limit.unwrap_or(50);
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
    presence_manager.join_drive(&drive_id).await;
    Ok(())
}

/// Leave a drive (remove presence)
#[tauri::command]
pub async fn leave_drive_presence(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<(), String> {
    presence_manager.leave_drive(&drive_id).await;
    Ok(())
}

/// Send a heartbeat to keep presence alive
#[tauri::command]
pub async fn presence_heartbeat(
    drive_id: String,
    presence_manager: State<'_, Arc<PresenceManager>>,
) -> Result<(), String> {
    let manager = presence_manager.get_drive_presence(&drive_id).await;
    let node_id = *presence_manager.node_id();
    manager.user_heartbeat(node_id).await;
    Ok(())
}
