//! Presence tracking for online users and activity feed
//!
//! Tracks which users are currently connected to a drive and
//! maintains an activity log of recent changes.

use crate::crypto::NodeId;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// User presence status
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PresenceStatus {
    /// User is actively connected
    Online,
    /// User was recently seen but may be idle
    Away,
    /// User is disconnected
    Offline,
}

/// Information about a connected user
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPresence {
    /// The user's node ID
    pub node_id: NodeId,
    /// Current status
    pub status: PresenceStatus,
    /// When they joined this drive
    pub joined_at: DateTime<Utc>,
    /// Last activity time
    pub last_seen: DateTime<Utc>,
    /// What they're currently doing (if known)
    pub current_activity: Option<String>,
}

impl UserPresence {
    pub fn new(node_id: NodeId) -> Self {
        let now = Utc::now();
        Self {
            node_id,
            status: PresenceStatus::Online,
            joined_at: now,
            last_seen: now,
            current_activity: None,
        }
    }

    /// Update last seen time
    pub fn touch(&mut self) {
        self.last_seen = Utc::now();
        self.status = PresenceStatus::Online;
    }

    /// Set current activity
    pub fn set_activity(&mut self, activity: Option<String>) {
        self.current_activity = activity;
        self.touch();
    }

    /// Check if user should be marked as away (5 min idle)
    pub fn check_idle(&mut self) {
        let idle_threshold = Duration::minutes(5);
        if Utc::now() - self.last_seen > idle_threshold {
            self.status = PresenceStatus::Away;
        }
    }
}

/// DTO for sending presence info to frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPresenceDto {
    pub node_id: String,
    pub short_id: String,
    pub status: String,
    pub joined_at: String,
    pub last_seen: String,
    pub current_activity: Option<String>,
    pub is_self: bool,
}

impl UserPresenceDto {
    pub fn from_presence(presence: &UserPresence, my_node_id: &NodeId) -> Self {
        Self {
            node_id: presence.node_id.to_hex(),
            short_id: presence.node_id.short_string(),
            status: format!("{:?}", presence.status).to_lowercase(),
            joined_at: presence.joined_at.to_rfc3339(),
            last_seen: presence.last_seen.to_rfc3339(),
            current_activity: presence.current_activity.clone(),
            is_self: presence.node_id == *my_node_id,
        }
    }
}

/// Type of activity that occurred
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActivityType {
    FileCreated,
    FileModified,
    FileDeleted,
    FileRenamed,
    UserJoined,
    UserLeft,
    LockAcquired,
    LockReleased,
    ConflictDetected,
    ConflictResolved,
}

/// An activity event in the feed
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Unique ID
    pub id: String,
    /// Type of activity
    pub activity_type: ActivityType,
    /// User who performed the action
    pub user: NodeId,
    /// File path (if applicable)
    pub path: Option<PathBuf>,
    /// When it happened
    pub timestamp: DateTime<Utc>,
    /// Additional details
    pub details: Option<String>,
}

impl ActivityEntry {
    pub fn new(activity_type: ActivityType, user: NodeId) -> Self {
        let id = Self::generate_id();
        Self {
            id,
            activity_type,
            user,
            path: None,
            timestamp: Utc::now(),
            details: None,
        }
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }

    fn generate_id() -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
        hasher.update(&rand::random::<[u8; 8]>());
        hex::encode(&hasher.finalize().as_bytes()[..8])
    }
}

/// DTO for sending activity to frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityEntryDto {
    pub id: String,
    pub activity_type: String,
    pub user_id: String,
    pub user_short: String,
    pub path: Option<String>,
    pub timestamp: String,
    pub details: Option<String>,
    pub is_self: bool,
}

impl ActivityEntryDto {
    pub fn from_entry(entry: &ActivityEntry, my_node_id: &NodeId) -> Self {
        Self {
            id: entry.id.clone(),
            activity_type: format!("{:?}", entry.activity_type),
            user_id: entry.user.to_hex(),
            user_short: entry.user.short_string(),
            path: entry.path.as_ref().map(|p| p.to_string_lossy().to_string()),
            timestamp: entry.timestamp.to_rfc3339(),
            details: entry.details.clone(),
            is_self: entry.user == *my_node_id,
        }
    }
}

/// Manages presence and activity for a single drive
#[derive(Debug)]
pub struct DrivePresenceManager {
    /// Connected users
    users: RwLock<HashMap<NodeId, UserPresence>>,
    /// Activity feed (newest first)
    activities: RwLock<Vec<ActivityEntry>>,
    /// Max activities to keep
    max_activities: usize,
}

impl DrivePresenceManager {
    pub fn new() -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            activities: RwLock::new(Vec::new()),
            max_activities: 200,
        }
    }

    /// Add or update a user's presence
    pub async fn user_joined(&self, node_id: NodeId) {
        let mut users = self.users.write().await;
        let is_new = !users.contains_key(&node_id);

        users
            .entry(node_id)
            .or_insert_with(|| UserPresence::new(node_id))
            .touch();

        // Only add activity if user actually joined (not already present)
        if is_new {
            drop(users); // Release lock before async call
            self.add_activity(ActivityEntry::new(ActivityType::UserJoined, node_id))
                .await;
        }
    }

    /// Remove a user
    pub async fn user_left(&self, node_id: NodeId) {
        let mut users = self.users.write().await;
        users.remove(&node_id);

        // Add activity
        self.add_activity(ActivityEntry::new(ActivityType::UserLeft, node_id))
            .await;
    }

    /// Update user's last seen
    pub async fn user_heartbeat(&self, node_id: NodeId) {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(&node_id) {
            user.touch();
        }
    }

    /// Get online users
    pub async fn online_users(&self) -> Vec<UserPresence> {
        let users = self.users.read().await;
        users.values().cloned().collect()
    }

    /// Get online count
    pub async fn online_count(&self) -> usize {
        let users = self.users.read().await;
        users.len()
    }

    /// Add an activity entry
    pub async fn add_activity(&self, entry: ActivityEntry) {
        let mut activities = self.activities.write().await;
        activities.insert(0, entry);

        // Trim to max
        if activities.len() > self.max_activities {
            activities.truncate(self.max_activities);
        }
    }

    /// Get recent activities
    pub async fn recent_activities(&self, limit: usize) -> Vec<ActivityEntry> {
        let activities = self.activities.read().await;
        activities.iter().take(limit).cloned().collect()
    }

    /// Get activities for a specific path
    pub async fn activities_for_path(&self, path: &PathBuf, limit: usize) -> Vec<ActivityEntry> {
        let activities = self.activities.read().await;
        activities
            .iter()
            .filter(|a| a.path.as_ref() == Some(path))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get activities for a specific user
    pub async fn activities_for_user(&self, user: &NodeId, limit: usize) -> Vec<ActivityEntry> {
        let activities = self.activities.read().await;
        activities
            .iter()
            .filter(|a| &a.user == user)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Check and update idle users
    pub async fn check_idle_users(&self) {
        let mut users = self.users.write().await;
        for user in users.values_mut() {
            user.check_idle();
        }
    }
}

impl Default for DrivePresenceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global presence manager for all drives
pub struct PresenceManager {
    /// Presence managers per drive (keyed by drive ID hex)
    drives: RwLock<HashMap<String, Arc<DrivePresenceManager>>>,
    /// Our node ID
    node_id: NodeId,
}

impl PresenceManager {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            drives: RwLock::new(HashMap::new()),
            node_id,
        }
    }

    /// Get or create presence manager for a drive
    pub async fn get_drive_presence(&self, drive_id: &str) -> Arc<DrivePresenceManager> {
        {
            let drives = self.drives.read().await;
            if let Some(manager) = drives.get(drive_id) {
                return manager.clone();
            }
        }

        let mut drives = self.drives.write().await;
        drives
            .entry(drive_id.to_string())
            .or_insert_with(|| Arc::new(DrivePresenceManager::new()))
            .clone()
    }

    /// Get our node ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Record that we joined a drive
    pub async fn join_drive(&self, drive_id: &str) {
        let manager = self.get_drive_presence(drive_id).await;
        manager.user_joined(self.node_id).await;
    }

    /// Record that we left a drive
    pub async fn leave_drive(&self, drive_id: &str) {
        let manager = self.get_drive_presence(drive_id).await;
        manager.user_left(self.node_id).await;
    }

    /// Get online users for a drive
    pub async fn get_online_users(&self, drive_id: &str) -> Vec<UserPresence> {
        let manager = self.get_drive_presence(drive_id).await;
        manager.online_users().await
    }

    /// Get recent activity for a drive
    pub async fn get_recent_activity(&self, drive_id: &str, limit: usize) -> Vec<ActivityEntry> {
        let manager = self.get_drive_presence(drive_id).await;
        manager.recent_activities(limit).await
    }

    /// Add an activity for a drive
    pub async fn add_activity(&self, drive_id: &str, entry: ActivityEntry) {
        let manager = self.get_drive_presence(drive_id).await;
        manager.add_activity(entry).await;
    }

    /// Cleanup old activities across all drives
    pub async fn cleanup_old_activities(&self, cutoff: DateTime<Utc>) -> usize {
        let drives = self.drives.read().await;
        let mut total = 0;
        for manager in drives.values() {
            let mut activities = manager.activities.write().await;
            let before = activities.len();
            activities.retain(|a| a.timestamp > cutoff);
            total += before - activities.len();
        }
        total
    }

    /// Update idle status for all users across all drives
    pub async fn update_idle_status(&self, _idle_threshold: Duration) -> usize {
        let drives = self.drives.read().await;
        let mut total = 0;
        for manager in drives.values() {
            manager.check_idle_users().await;
            // Count how many were marked idle/offline
            let users = manager.online_users().await;
            for user in users {
                if user.status != PresenceStatus::Online {
                    total += 1;
                }
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Identity;

    #[tokio::test]
    async fn test_user_presence() {
        let identity = Identity::generate();
        let node_id = identity.node_id();

        let manager = DrivePresenceManager::new();
        manager.user_joined(node_id).await;

        let users = manager.online_users().await;
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].node_id, node_id);
        assert_eq!(users[0].status, PresenceStatus::Online);
    }

    #[tokio::test]
    async fn test_activity_feed() {
        let identity = Identity::generate();
        let node_id = identity.node_id();

        let manager = DrivePresenceManager::new();

        manager
            .add_activity(
                ActivityEntry::new(ActivityType::FileCreated, node_id)
                    .with_path(PathBuf::from("test.txt")),
            )
            .await;

        let activities = manager.recent_activities(10).await;
        assert_eq!(activities.len(), 1);
        assert!(matches!(activities[0].activity_type, ActivityType::FileCreated));
    }
}
