//! Conflict detection and resolution for file synchronization
//!
//! Detects when multiple peers modify the same file simultaneously
//! and provides resolution strategies.

use crate::crypto::NodeId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Conflict resolution strategy
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResolutionStrategy {
    /// Keep local version, discard remote
    KeepLocal,
    /// Keep remote version, discard local
    KeepRemote,
    /// Keep both versions (rename conflicting file)
    KeepBoth,
    /// Manual merge required (for text files)
    ManualMerge,
}

/// Information about a conflicting version
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictVersion {
    /// BLAKE3 hash of the content
    pub hash: String,
    /// Size in bytes
    pub size: u64,
    /// Last modified time
    pub modified_at: DateTime<Utc>,
    /// Node that made the change
    pub modified_by: NodeId,
    /// Optional preview/snippet for text files
    pub preview: Option<String>,
}

/// Represents a file conflict
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileConflict {
    /// Unique conflict ID
    pub id: String,
    /// Path relative to drive root
    pub path: PathBuf,
    /// When the conflict was detected
    pub detected_at: DateTime<Utc>,
    /// Local version info
    pub local: ConflictVersion,
    /// Remote version info
    pub remote: ConflictVersion,
    /// Common ancestor hash (if known)
    pub base_hash: Option<String>,
    /// Whether this conflict has been resolved
    pub resolved: bool,
    /// Resolution used (if resolved)
    pub resolution: Option<ResolutionStrategy>,
}

impl FileConflict {
    /// Create a new conflict
    pub fn new(
        path: PathBuf,
        local: ConflictVersion,
        remote: ConflictVersion,
        base_hash: Option<String>,
    ) -> Self {
        let id = Self::generate_id(&path, &local.hash, &remote.hash);
        Self {
            id,
            path,
            detected_at: Utc::now(),
            local,
            remote,
            base_hash,
            resolved: false,
            resolution: None,
        }
    }

    /// Generate a deterministic conflict ID
    fn generate_id(path: &Path, local_hash: &str, remote_hash: &str) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(local_hash.as_bytes());
        hasher.update(remote_hash.as_bytes());
        hex::encode(&hasher.finalize().as_bytes()[..16])
    }

    /// Mark as resolved
    pub fn resolve(&mut self, strategy: ResolutionStrategy) {
        self.resolved = true;
        self.resolution = Some(strategy);
    }

    /// Check if file is a text file (can be merged)
    pub fn is_text_file(&self) -> bool {
        let ext = self
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(
            ext.as_str(),
            "txt"
                | "md"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "xml"
                | "html"
                | "css"
                | "js"
                | "ts"
                | "jsx"
                | "tsx"
                | "rs"
                | "py"
                | "go"
                | "java"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "sh"
                | "bash"
                | "zsh"
                | "ps1"
                | "bat"
                | "cmd"
                | "sql"
                | "csv"
        )
    }

    /// Get suggested resolution strategy
    pub fn suggested_resolution(&self) -> ResolutionStrategy {
        // If remote is newer, suggest keeping remote
        if self.remote.modified_at > self.local.modified_at {
            ResolutionStrategy::KeepRemote
        } else if self.local.modified_at > self.remote.modified_at {
            ResolutionStrategy::KeepLocal
        } else {
            // Same time, suggest keeping both
            ResolutionStrategy::KeepBoth
        }
    }
}

/// DTO for sending conflict info to frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileConflictDto {
    pub id: String,
    pub path: String,
    pub detected_at: String,
    pub local_hash: String,
    pub local_size: u64,
    pub local_modified_at: String,
    pub local_modified_by: String,
    pub local_preview: Option<String>,
    pub remote_hash: String,
    pub remote_size: u64,
    pub remote_modified_at: String,
    pub remote_modified_by: String,
    pub remote_preview: Option<String>,
    pub is_text_file: bool,
    pub suggested_resolution: String,
    pub resolved: bool,
}

impl From<&FileConflict> for FileConflictDto {
    fn from(conflict: &FileConflict) -> Self {
        Self {
            id: conflict.id.clone(),
            path: conflict.path.to_string_lossy().to_string(),
            detected_at: conflict.detected_at.to_rfc3339(),
            local_hash: conflict.local.hash.clone(),
            local_size: conflict.local.size,
            local_modified_at: conflict.local.modified_at.to_rfc3339(),
            local_modified_by: conflict.local.modified_by.to_hex(),
            local_preview: conflict.local.preview.clone(),
            remote_hash: conflict.remote.hash.clone(),
            remote_size: conflict.remote.size,
            remote_modified_at: conflict.remote.modified_at.to_rfc3339(),
            remote_modified_by: conflict.remote.modified_by.to_hex(),
            remote_preview: conflict.remote.preview.clone(),
            is_text_file: conflict.is_text_file(),
            suggested_resolution: format!("{:?}", conflict.suggested_resolution()),
            resolved: conflict.resolved,
        }
    }
}

/// Manages conflicts for a single drive
#[derive(Debug)]
pub struct DriveConflictManager {
    /// Active conflicts keyed by path
    conflicts: RwLock<HashMap<PathBuf, FileConflict>>,
    /// Resolved conflicts (kept for history)
    resolved: RwLock<Vec<FileConflict>>,
}

impl DriveConflictManager {
    pub fn new() -> Self {
        Self {
            conflicts: RwLock::new(HashMap::new()),
            resolved: RwLock::new(Vec::new()),
        }
    }

    /// Add a new conflict
    pub async fn add_conflict(&self, conflict: FileConflict) {
        let mut conflicts = self.conflicts.write().await;
        conflicts.insert(conflict.path.clone(), conflict);
    }

    /// Get a conflict by path
    pub async fn get_conflict(&self, path: &PathBuf) -> Option<FileConflict> {
        let conflicts = self.conflicts.read().await;
        conflicts.get(path).cloned()
    }

    /// Get a conflict by ID
    pub async fn get_conflict_by_id(&self, id: &str) -> Option<FileConflict> {
        let conflicts = self.conflicts.read().await;
        conflicts.values().find(|c| c.id == id).cloned()
    }

    /// List all unresolved conflicts
    pub async fn list_conflicts(&self) -> Vec<FileConflict> {
        let conflicts = self.conflicts.read().await;
        conflicts.values().cloned().collect()
    }

    /// Resolve a conflict
    pub async fn resolve_conflict(
        &self,
        path: &PathBuf,
        strategy: ResolutionStrategy,
    ) -> Option<FileConflict> {
        let mut conflicts = self.conflicts.write().await;

        if let Some(mut conflict) = conflicts.remove(path) {
            conflict.resolve(strategy);

            // Add to resolved history
            let mut resolved = self.resolved.write().await;
            resolved.push(conflict.clone());

            // Keep only last 100 resolved conflicts
            if resolved.len() > 100 {
                resolved.remove(0);
            }

            return Some(conflict);
        }

        None
    }

    /// Get count of unresolved conflicts
    pub async fn conflict_count(&self) -> usize {
        let conflicts = self.conflicts.read().await;
        conflicts.len()
    }

    /// Clear all conflicts (use with caution)
    pub async fn clear_all(&self) {
        let mut conflicts = self.conflicts.write().await;
        conflicts.clear();
    }
}

impl Default for DriveConflictManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global conflict manager for all drives
pub struct ConflictManager {
    /// Conflict managers per drive (keyed by drive ID hex)
    drives: RwLock<HashMap<String, Arc<DriveConflictManager>>>,
}

impl ConflictManager {
    pub fn new() -> Self {
        Self {
            drives: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create conflict manager for a drive
    pub async fn get_drive_conflicts(&self, drive_id: &str) -> Arc<DriveConflictManager> {
        {
            let drives = self.drives.read().await;
            if let Some(manager) = drives.get(drive_id) {
                return manager.clone();
            }
        }

        let mut drives = self.drives.write().await;
        drives
            .entry(drive_id.to_string())
            .or_insert_with(|| Arc::new(DriveConflictManager::new()))
            .clone()
    }

    /// Detect if incoming change conflicts with local state
    pub async fn detect_conflict(
        &self,
        drive_id: &str,
        path: PathBuf,
        local: ConflictVersion,
        remote: ConflictVersion,
        base_hash: Option<String>,
    ) -> Option<FileConflict> {
        // No conflict if hashes are the same
        if local.hash == remote.hash {
            return None;
        }

        // No conflict if local hasn't been modified since base
        if let Some(ref base) = base_hash {
            if local.hash == *base {
                // Local unchanged, remote wins
                return None;
            }
            if remote.hash == *base {
                // Remote unchanged, local wins
                return None;
            }
        }

        // Both changed - this is a conflict
        let conflict = FileConflict::new(path, local, remote, base_hash);
        let manager = self.get_drive_conflicts(drive_id).await;
        manager.add_conflict(conflict.clone()).await;

        Some(conflict)
    }

    /// List conflicts for a drive
    pub async fn list_conflicts(&self, drive_id: &str) -> Vec<FileConflict> {
        let manager = self.get_drive_conflicts(drive_id).await;
        manager.list_conflicts().await
    }

    /// Resolve a conflict
    pub async fn resolve_conflict(
        &self,
        drive_id: &str,
        path: &PathBuf,
        strategy: ResolutionStrategy,
    ) -> Option<FileConflict> {
        let manager = self.get_drive_conflicts(drive_id).await;
        manager.resolve_conflict(path, strategy).await
    }

    /// Get total conflict count across all drives
    pub async fn total_conflict_count(&self) -> usize {
        let drives = self.drives.read().await;
        let mut total = 0;
        for manager in drives.values() {
            total += manager.conflict_count().await;
        }
        total
    }

    /// Cleanup old resolved conflicts across all drives
    pub async fn cleanup_old_resolved(&self, cutoff: DateTime<Utc>) -> usize {
        let drives = self.drives.read().await;
        let mut total = 0;
        for manager in drives.values() {
            let mut resolved = manager.resolved.write().await;
            let before = resolved.len();
            resolved.retain(|c| c.detected_at > cutoff);
            total += before - resolved.len();
        }
        total
    }
}

impl Default for ConflictManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Identity;

    #[tokio::test]
    async fn test_conflict_detection() {
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();

        let local = ConflictVersion {
            hash: "local123".to_string(),
            size: 100,
            modified_at: Utc::now(),
            modified_by: identity1.node_id(),
            preview: None,
        };

        let remote = ConflictVersion {
            hash: "remote456".to_string(),
            size: 150,
            modified_at: Utc::now(),
            modified_by: identity2.node_id(),
            preview: None,
        };

        let conflict = FileConflict::new(
            PathBuf::from("test/file.txt"),
            local,
            remote,
            Some("base789".to_string()),
        );

        assert!(!conflict.resolved);
        assert!(conflict.is_text_file());
    }

    #[tokio::test]
    async fn test_conflict_manager() {
        let manager = ConflictManager::new();
        let identity1 = Identity::generate();
        let identity2 = Identity::generate();

        let local = ConflictVersion {
            hash: "local123".to_string(),
            size: 100,
            modified_at: Utc::now(),
            modified_by: identity1.node_id(),
            preview: None,
        };

        let remote = ConflictVersion {
            hash: "remote456".to_string(),
            size: 150,
            modified_at: Utc::now(),
            modified_by: identity2.node_id(),
            preview: None,
        };

        let conflict = manager
            .detect_conflict(
                "drive123",
                PathBuf::from("test/file.txt"),
                local,
                remote,
                Some("base789".to_string()),
            )
            .await;

        assert!(conflict.is_some());

        let conflicts = manager.list_conflicts("drive123").await;
        assert_eq!(conflicts.len(), 1);

        // Resolve
        let resolved = manager
            .resolve_conflict(
                "drive123",
                &PathBuf::from("test/file.txt"),
                ResolutionStrategy::KeepLocal,
            )
            .await;

        assert!(resolved.is_some());
        assert!(resolved.unwrap().resolved);

        // Should be empty now
        let conflicts = manager.list_conflicts("drive123").await;
        assert_eq!(conflicts.len(), 0);
    }
}
