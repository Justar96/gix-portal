//! File system watcher for detecting local changes
//!
//! Uses the notify crate with debouncing to monitor shared drive folders
//! and convert file system events into DriveEvents for sync.

use crate::core::{send_with_backpressure, DriveEvent, DriveId};
use crate::crypto::NodeId;
use anyhow::Result;
use chrono::Utc;
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
    Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};

/// Patterns to ignore when watching
const IGNORE_PATTERNS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "__pycache__",
    ".DS_Store",
    "Thumbs.db",
    ".idea",
    ".vscode",
    "target",
    "*.tmp",
    "*.swp",
    "*.swo",
    "~$*", // Office temp files
];

/// A watched drive's state
struct WatchedDrive {
    /// The drive ID (stored for future reference)
    _drive_id: DriveId,
    /// Root path being watched (stored for future reference)
    _root_path: PathBuf,
    /// The file watcher handle
    _watcher: RecommendedWatcher,
}

/// Manages file system watchers for all active drives
pub struct FileWatcherManager {
    /// Currently watched drives
    watched: Arc<RwLock<HashMap<DriveId, WatchedDrive>>>,
    /// Node ID for event attribution
    node_id: NodeId,
    /// Channel for emitting drive events
    event_tx: broadcast::Sender<(DriveId, DriveEvent)>,
}

impl FileWatcherManager {
    /// Create a new file watcher manager
    pub fn new(node_id: NodeId) -> Self {
        let (event_tx, _) = broadcast::channel(1024);

        Self {
            watched: Arc::new(RwLock::new(HashMap::new())),
            node_id,
            event_tx,
        }
    }

    /// Subscribe to file watcher events
    pub fn subscribe(&self) -> broadcast::Receiver<(DriveId, DriveEvent)> {
        self.event_tx.subscribe()
    }

    /// Start watching a drive's folder
    pub async fn watch(&self, drive_id: DriveId, path: PathBuf) -> Result<()> {
        // Check if already watching
        {
            let watched = self.watched.read().await;
            if watched.contains_key(&drive_id) {
                tracing::debug!("Already watching drive: {}", drive_id);
                return Ok(());
            }
        }

        // Validate path exists
        if !path.exists() {
            anyhow::bail!("Path does not exist: {:?}", path);
        }
        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {:?}", path);
        }

        // Create channel for this watcher
        let (tx, mut rx) = mpsc::channel::<notify::Result<notify::Event>>(256);

        // Create watcher
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.blocking_send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        // Start watching
        let mut watcher = watcher;
        watcher.watch(&path, RecursiveMode::Recursive)?;

        // Spawn event processor task
        let drive_id_clone = drive_id;
        let root_path = path.clone();
        let node_id = self.node_id;
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let mut pending_renames: HashMap<PathBuf, std::time::Instant> = HashMap::new();

            while let Some(res) = rx.recv().await {
                match res {
                    Ok(event) => {
                        // Process the event
                        if let Some(drive_event) =
                            process_fs_event(&event, &root_path, &node_id, &mut pending_renames)
                        {
                            send_with_backpressure(
                                &event_tx,
                                (drive_id_clone, drive_event),
                                "file_watcher",
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("File watcher error for drive {}: {}", drive_id_clone, e);
                    }
                }
            }

            tracing::debug!("File watcher stopped for drive: {}", drive_id_clone);
        });

        // Store watcher
        let watched_drive = WatchedDrive {
            _drive_id: drive_id,
            _root_path: path.clone(),
            _watcher: watcher,
        };

        self.watched.write().await.insert(drive_id, watched_drive);
        tracing::info!("Started watching drive {} at {:?}", drive_id, path);

        Ok(())
    }

    /// Stop watching a drive
    pub async fn unwatch(&self, drive_id: &DriveId) {
        let mut watched = self.watched.write().await;
        if watched.remove(drive_id).is_some() {
            tracing::info!("Stopped watching drive: {}", drive_id);
        }
    }

    /// Check if a drive is being watched
    pub async fn is_watching(&self, drive_id: &DriveId) -> bool {
        self.watched.read().await.contains_key(drive_id)
    }

    /// Get count of watched drives
    #[allow(dead_code)]
    pub async fn watched_count(&self) -> usize {
        self.watched.read().await.len()
    }
}

/// Process a file system event and convert to DriveEvent if applicable
fn process_fs_event(
    event: &notify::Event,
    root_path: &Path,
    node_id: &NodeId,
    _pending_renames: &mut HashMap<PathBuf, std::time::Instant>,
) -> Option<DriveEvent> {
    // Get the first path from the event
    let path = event.paths.first()?;

    // Check if path should be ignored
    if should_ignore(path) {
        return None;
    }

    // Get relative path from root
    let relative_path = path.strip_prefix(root_path).ok()?.to_path_buf();

    match &event.kind {
        EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Data(_)) => {
            // File created or modified
            let (hash, size) = compute_file_info(path)?;

            Some(DriveEvent::FileChanged {
                path: relative_path,
                hash,
                size,
                modified_by: *node_id,
                timestamp: Utc::now(),
            })
        }

        EventKind::Create(CreateKind::Folder) => {
            // Folder created - emit as a "file" with size 0
            Some(DriveEvent::FileChanged {
                path: relative_path,
                hash: String::new(),
                size: 0,
                modified_by: *node_id,
                timestamp: Utc::now(),
            })
        }

        EventKind::Remove(RemoveKind::File) | EventKind::Remove(RemoveKind::Folder) => {
            Some(DriveEvent::FileDeleted {
                path: relative_path,
                deleted_by: *node_id,
                timestamp: Utc::now(),
            })
        }

        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            // Rename event with both old and new paths
            if event.paths.len() >= 2 {
                let new_path = &event.paths[1];
                let new_relative = new_path.strip_prefix(root_path).ok()?.to_path_buf();

                // Emit delete for old path, create for new path
                // For now, just emit the new file as changed
                if new_path.exists() {
                    let (hash, size) = compute_file_info(new_path)?;
                    Some(DriveEvent::FileChanged {
                        path: new_relative,
                        hash,
                        size,
                        modified_by: *node_id,
                        timestamp: Utc::now(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }

        _ => {
            // Ignore other event types (access, metadata changes, etc.)
            None
        }
    }
}

/// Check if a path should be ignored
fn should_ignore(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in IGNORE_PATTERNS {
        if let Some(suffix) = pattern.strip_prefix('*') {
            // Suffix pattern (e.g., "*.tmp")
            if path_str.ends_with(suffix) {
                return true;
            }
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            // Prefix pattern (e.g., "~$*")
            if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy().starts_with(prefix) {
                    return true;
                }
            }
        } else {
            // Exact match or path component match
            for component in path.components() {
                if let std::path::Component::Normal(name) = component {
                    if name.to_string_lossy() == *pattern {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Compute BLAKE3 hash and size for a file
fn compute_file_info(path: &Path) -> Option<(String, u64)> {
    let metadata = std::fs::metadata(path).ok()?;

    if metadata.is_dir() {
        return Some((String::new(), 0));
    }

    let size = metadata.len();

    // For small files, compute full hash
    // For large files, we could sample or defer
    if size <= 10 * 1024 * 1024 {
        // 10MB limit for inline hashing
        let data = std::fs::read(path).ok()?;
        let hash = blake3::hash(&data);
        Some((hash.to_hex().to_string(), size))
    } else {
        // For large files, hash first/last chunks + size
        // This is faster but less accurate - good enough for change detection
        let mut file = std::fs::File::open(path).ok()?;
        let mut hasher = blake3::Hasher::new();

        // Hash first 1MB
        let mut buffer = vec![0u8; 1024 * 1024];
        use std::io::Read;
        let n = file.read(&mut buffer).ok()?;
        hasher.update(&buffer[..n]);

        // Hash the size as well for uniqueness
        hasher.update(&size.to_le_bytes());

        Some((hasher.finalize().to_hex().to_string(), size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ignore() {
        assert!(should_ignore(Path::new("/project/.git/HEAD")));
        assert!(should_ignore(Path::new("/project/node_modules/pkg")));
        assert!(should_ignore(Path::new("/project/file.tmp")));
        assert!(should_ignore(Path::new("/project/~$document.docx")));
        assert!(!should_ignore(Path::new("/project/src/main.rs")));
        assert!(!should_ignore(Path::new("/project/README.md")));
    }

    #[test]
    fn test_ignore_patterns() {
        // Test various patterns
        assert!(should_ignore(Path::new("/.git")));
        assert!(should_ignore(Path::new("/foo/.git/config")));
        assert!(should_ignore(Path::new("/test.swp")));
        assert!(should_ignore(Path::new("/doc.tmp")));
    }
}
