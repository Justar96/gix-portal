//! File locking for collaborative editing
//!
//! Provides advisory and exclusive locking to prevent edit conflicts.
//! Locks are broadcast via gossip so all peers see lock status.

use crate::crypto::NodeId;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Type of lock that can be acquired
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LockType {
    /// Advisory lock - warns others but doesn't prevent access
    Advisory,
    /// Exclusive lock - prevents others from editing
    Exclusive,
}

/// Represents an active lock on a file
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileLock {
    /// Path relative to drive root
    pub path: PathBuf,
    /// Node that holds the lock
    pub holder: NodeId,
    /// Type of lock
    pub lock_type: LockType,
    /// When the lock was acquired
    pub acquired_at: DateTime<Utc>,
    /// When the lock expires (auto-release)
    pub expires_at: DateTime<Utc>,
    /// Optional reason for the lock
    pub reason: Option<String>,
}

impl FileLock {
    /// Create a new lock with default 30-minute expiration
    pub fn new(path: PathBuf, holder: NodeId, lock_type: LockType) -> Self {
        let now = Utc::now();
        Self {
            path,
            holder,
            lock_type,
            acquired_at: now,
            expires_at: now + Duration::minutes(30),
            reason: None,
        }
    }

    /// Create a lock with custom expiration
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = expires_at;
        self
    }

    /// Add a reason for the lock
    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Check if the lock has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Extend the lock by a duration
    pub fn extend(&mut self, duration: Duration) {
        self.expires_at = Utc::now() + duration;
    }

    /// Check if this lock is held by the given node
    pub fn is_held_by(&self, node_id: &NodeId) -> bool {
        self.holder == *node_id
    }
}

/// DTO for sending lock info to frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileLockDto {
    pub path: String,
    pub holder: String,
    pub lock_type: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub reason: Option<String>,
    pub is_mine: bool,
}

impl FileLockDto {
    pub fn from_lock(lock: &FileLock, my_node_id: &NodeId) -> Self {
        Self {
            path: lock.path.to_string_lossy().to_string(),
            holder: lock.holder.to_hex(),
            lock_type: match lock.lock_type {
                LockType::Advisory => "advisory".to_string(),
                LockType::Exclusive => "exclusive".to_string(),
            },
            acquired_at: lock.acquired_at.to_rfc3339(),
            expires_at: lock.expires_at.to_rfc3339(),
            reason: lock.reason.clone(),
            is_mine: lock.is_held_by(my_node_id),
        }
    }
}

/// Result of attempting to acquire a lock
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LockResult {
    /// Lock acquired successfully
    Acquired(FileLock),
    /// Lock denied - file already locked by another user
    Denied {
        existing_lock: FileLock,
        reason: String,
    },
    /// Lock acquired but with a warning (e.g., advisory lock exists)
    AcquiredWithWarning { lock: FileLock, warning: String },
}

/// Manages file locks for a single drive
#[derive(Debug)]
pub struct DriveLockManager {
    /// Active locks keyed by file path
    locks: RwLock<HashMap<PathBuf, FileLock>>,
}

impl DriveLockManager {
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
        }
    }

    /// Attempt to acquire a lock
    pub async fn acquire(&self, path: PathBuf, holder: NodeId, lock_type: LockType) -> LockResult {
        let mut locks = self.locks.write().await;

        // Clean up expired locks first
        locks.retain(|_, lock| !lock.is_expired());

        // Check if already locked
        if let Some(existing) = locks.get(&path) {
            // Same holder can upgrade/refresh their lock
            if existing.holder == holder {
                let new_lock = FileLock::new(path.clone(), holder, lock_type);
                locks.insert(path, new_lock.clone());
                return LockResult::Acquired(new_lock);
            }

            // Different holder
            match (existing.lock_type, lock_type) {
                // Exclusive lock blocks everything
                (LockType::Exclusive, _) => {
                    return LockResult::Denied {
                        existing_lock: existing.clone(),
                        reason: format!(
                            "File is exclusively locked by {}",
                            existing.holder.short_string()
                        ),
                    };
                }
                // Trying to get exclusive on advisory
                (LockType::Advisory, LockType::Exclusive) => {
                    return LockResult::Denied {
                        existing_lock: existing.clone(),
                        reason: format!(
                            "File has advisory lock by {} - cannot acquire exclusive",
                            existing.holder.short_string()
                        ),
                    };
                }
                // Advisory on advisory - warn but allow
                (LockType::Advisory, LockType::Advisory) => {
                    let new_lock = FileLock::new(path.clone(), holder, lock_type);
                    // Don't replace existing advisory lock, just warn
                    return LockResult::AcquiredWithWarning {
                        lock: new_lock,
                        warning: format!(
                            "File also has advisory lock by {}",
                            existing.holder.short_string()
                        ),
                    };
                }
            }
        }

        // No existing lock, acquire it
        let lock = FileLock::new(path.clone(), holder, lock_type);
        locks.insert(path, lock.clone());
        LockResult::Acquired(lock)
    }

    /// Release a lock
    pub async fn release(&self, path: &PathBuf, holder: &NodeId) -> Option<FileLock> {
        let mut locks = self.locks.write().await;

        if let Some(existing) = locks.get(path) {
            if existing.holder == *holder {
                return locks.remove(path);
            }
        }
        None
    }

    /// Force release a lock (for admins)
    pub async fn force_release(&self, path: &PathBuf) -> Option<FileLock> {
        let mut locks = self.locks.write().await;
        locks.remove(path)
    }

    /// Get lock status for a path
    pub async fn get_lock(&self, path: &PathBuf) -> Option<FileLock> {
        let locks = self.locks.read().await;
        locks.get(path).filter(|l| !l.is_expired()).cloned()
    }

    /// Get all active locks
    pub async fn list_locks(&self) -> Vec<FileLock> {
        let locks = self.locks.read().await;
        locks
            .values()
            .filter(|l| !l.is_expired())
            .cloned()
            .collect()
    }

    /// Extend a lock
    pub async fn extend_lock(
        &self,
        path: &PathBuf,
        holder: &NodeId,
        duration_mins: i64,
    ) -> Option<FileLock> {
        let mut locks = self.locks.write().await;

        if let Some(lock) = locks.get_mut(path) {
            if lock.holder == *holder && !lock.is_expired() {
                lock.extend(Duration::minutes(duration_mins));
                return Some(lock.clone());
            }
        }
        None
    }

    /// Apply a remote lock (from gossip)
    pub async fn apply_remote_lock(&self, lock: FileLock) {
        if lock.is_expired() {
            return;
        }

        let mut locks = self.locks.write().await;

        // Only apply if no existing lock or existing lock is expired
        if let Some(existing) = locks.get(&lock.path) {
            if !existing.is_expired() && existing.acquired_at < lock.acquired_at {
                // Existing lock is older, keep it
                return;
            }
        }

        locks.insert(lock.path.clone(), lock);
    }

    /// Remove a remote lock (from gossip)
    pub async fn remove_remote_lock(&self, path: &PathBuf, holder: &NodeId) {
        let mut locks = self.locks.write().await;

        if let Some(existing) = locks.get(path) {
            if existing.holder == *holder {
                locks.remove(path);
            }
        }
    }

    /// Cleanup expired locks
    pub async fn cleanup_expired(&self) -> usize {
        let mut locks = self.locks.write().await;
        let before = locks.len();
        locks.retain(|_, lock| !lock.is_expired());
        before - locks.len()
    }
}

impl Default for DriveLockManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global lock manager for all drives
pub struct LockManager {
    /// Lock managers per drive (keyed by drive ID hex)
    drives: RwLock<HashMap<String, Arc<DriveLockManager>>>,
    /// Our node ID for ownership checks
    node_id: NodeId,
}

impl LockManager {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            drives: RwLock::new(HashMap::new()),
            node_id,
        }
    }

    /// Get or create lock manager for a drive
    pub async fn get_drive_locks(&self, drive_id: &str) -> Arc<DriveLockManager> {
        {
            let drives = self.drives.read().await;
            if let Some(manager) = drives.get(drive_id) {
                return manager.clone();
            }
        }

        let mut drives = self.drives.write().await;
        drives
            .entry(drive_id.to_string())
            .or_insert_with(|| Arc::new(DriveLockManager::new()))
            .clone()
    }

    /// Get our node ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Acquire a lock
    pub async fn acquire_lock(
        &self,
        drive_id: &str,
        path: PathBuf,
        lock_type: LockType,
    ) -> LockResult {
        let manager = self.get_drive_locks(drive_id).await;
        manager.acquire(path, self.node_id, lock_type).await
    }

    /// Release a lock
    pub async fn release_lock(&self, drive_id: &str, path: &PathBuf) -> Option<FileLock> {
        let manager = self.get_drive_locks(drive_id).await;
        manager.release(path, &self.node_id).await
    }

    /// Get lock info
    pub async fn get_lock(&self, drive_id: &str, path: &PathBuf) -> Option<FileLock> {
        let manager = self.get_drive_locks(drive_id).await;
        manager.get_lock(path).await
    }

    /// List all locks for a drive
    pub async fn list_locks(&self, drive_id: &str) -> Vec<FileLock> {
        let manager = self.get_drive_locks(drive_id).await;
        manager.list_locks().await
    }

    /// Extend a lock
    pub async fn extend_lock(
        &self,
        drive_id: &str,
        path: &PathBuf,
        duration_mins: i64,
    ) -> Option<FileLock> {
        let manager = self.get_drive_locks(drive_id).await;
        manager
            .extend_lock(path, &self.node_id, duration_mins)
            .await
    }

    /// Apply a lock received from gossip
    pub async fn apply_remote_lock(&self, drive_id: &str, lock: FileLock) {
        let manager = self.get_drive_locks(drive_id).await;
        manager.apply_remote_lock(lock).await;
    }

    /// Remove a lock received from gossip
    pub async fn remove_remote_lock(&self, drive_id: &str, path: &PathBuf, holder: &NodeId) {
        let manager = self.get_drive_locks(drive_id).await;
        manager.remove_remote_lock(path, holder).await;
    }

    /// Cleanup expired locks across all drives
    pub async fn cleanup_expired(&self) -> usize {
        let drives = self.drives.read().await;
        let mut total = 0;
        for manager in drives.values() {
            total += manager.cleanup_expired().await;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Identity;

    #[tokio::test]
    async fn test_lock_acquire_release() {
        let identity = Identity::generate();
        let node_id = identity.node_id();
        let manager = DriveLockManager::new();
        let path = PathBuf::from("test/file.txt");

        // Acquire lock
        let result = manager
            .acquire(path.clone(), node_id, LockType::Advisory)
            .await;
        assert!(matches!(result, LockResult::Acquired(_)));

        // Release lock
        let released = manager.release(&path, &node_id).await;
        assert!(released.is_some());

        // Verify released
        assert!(manager.get_lock(&path).await.is_none());
    }

    #[tokio::test]
    async fn test_exclusive_lock_blocks() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();
        let node1 = identity1.node_id();
        let node2 = identity2.node_id();
        let manager = DriveLockManager::new();
        let path = PathBuf::from("test/file.txt");

        // User 1 acquires exclusive lock
        let result = manager
            .acquire(path.clone(), node1, LockType::Exclusive)
            .await;
        assert!(matches!(result, LockResult::Acquired(_)));

        // User 2 cannot acquire any lock
        let result = manager
            .acquire(path.clone(), node2, LockType::Advisory)
            .await;
        assert!(matches!(result, LockResult::Denied { .. }));
    }

    #[tokio::test]
    async fn test_lock_expiration() {
        let identity = Identity::generate();
        let node_id = identity.node_id();
        let manager = DriveLockManager::new();
        let path = PathBuf::from("test/file.txt");

        // Create an expired lock
        let mut lock = FileLock::new(path.clone(), node_id, LockType::Advisory);
        lock.expires_at = Utc::now() - Duration::minutes(5);

        {
            let mut locks = manager.locks.write().await;
            locks.insert(path.clone(), lock);
        }

        // Should not return expired lock
        assert!(manager.get_lock(&path).await.is_none());
    }
}
