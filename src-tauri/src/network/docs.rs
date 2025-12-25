//! Document synchronization via iroh-docs protocol
//!
//! Provides CRDT-based metadata synchronization for drives.
//! Each drive has its own iroh-docs document (namespace) that stores
//! file metadata and syncs automatically between peers.
//!
//! Metadata is persisted to database and synced via gossip.

#![allow(dead_code)]

use crate::core::DriveId;
use crate::storage::Database;
use anyhow::Result;
use iroh_docs::{AuthorId, DocTicket, NamespaceId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Metadata schema stored in iroh-docs
/// Key format: "file:{relative_path}"
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File name
    pub name: String,
    /// Relative path within drive
    pub path: String,
    /// Is this a directory?
    pub is_dir: bool,
    /// File size in bytes
    pub size: u64,
    /// ISO 8601 modified timestamp
    pub modified_at: String,
    /// BLAKE3 content hash (hex string, for file transfer)
    pub content_hash: Option<String>,
    /// Monotonic version number for conflict resolution
    pub version: u64,
}

impl FileMetadata {
    /// Create a new file metadata entry
    pub fn new(path: &str, name: &str, is_dir: bool, size: u64, modified_at: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            is_dir,
            size,
            modified_at: modified_at.to_string(),
            content_hash: None,
            version: 1,
        }
    }

    /// Create with content hash
    pub fn with_hash(path: &str, name: &str, is_dir: bool, size: u64, modified_at: &str, hash: String) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            is_dir,
            size,
            modified_at: modified_at.to_string(),
            content_hash: Some(hash),
            version: 1,
        }
    }

    /// Generate the iroh-docs key for this entry
    pub fn doc_key(&self) -> Vec<u8> {
        format!("file:{}", self.path).into_bytes()
    }
}

/// Placeholder author ID until full iroh-docs integration
#[derive(Clone, Copy, Debug)]
pub struct PlaceholderAuthorId([u8; 32]);

impl std::fmt::Display for PlaceholderAuthorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..8]))
    }
}

/// Manages document metadata for drives
///
/// Stores metadata in database for persistence and in memory for fast access.
/// Syncs with peers via gossip protocol.
pub struct DocsManager {
    /// Database for persistence
    db: Arc<Database>,
    /// Our author identity ID (placeholder until full integration)
    _author_id: PlaceholderAuthorId,
    /// Mapping from DriveId to document NamespaceId
    namespaces: RwLock<HashMap<DriveId, NamespaceId>>,
    /// In-memory metadata cache per drive (for fast lookups)
    metadata_cache: RwLock<HashMap<DriveId, HashMap<String, FileMetadata>>>,
    /// Data directory for persistent storage
    #[allow(dead_code)]
    data_dir: PathBuf,
}

impl DocsManager {
    /// Create a new DocsManager with database persistence
    pub async fn new(data_dir: &std::path::Path, db: Arc<Database>) -> Result<Self> {
        // Create directories for docs storage
        let docs_dir = data_dir.join("docs");
        std::fs::create_dir_all(&docs_dir)?;

        // Generate a placeholder author ID
        let mut author_bytes = [0u8; 32];
        author_bytes.copy_from_slice(&rand::random::<[u8; 32]>());
        let author_id = PlaceholderAuthorId(author_bytes);

        tracing::info!(
            "DocsManager initialized with placeholder author: {}",
            author_id
        );

        Ok(Self {
            db,
            _author_id: author_id,
            namespaces: RwLock::new(HashMap::new()),
            metadata_cache: RwLock::new(HashMap::new()),
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Load metadata from database for a drive
    pub async fn load_drive_metadata(&self, drive_id: &DriveId) -> Result<()> {
        let drive_id_hex = hex::encode(drive_id.as_bytes());
        
        let metadata_list = self.db.list_file_metadata(&drive_id_hex)?;
        
        let mut cache = self.metadata_cache.write().await;
        let drive_cache = cache.entry(*drive_id).or_insert_with(HashMap::new);
        
        for (path, data) in metadata_list {
            match serde_json::from_slice::<FileMetadata>(&data) {
                Ok(meta) => {
                    drive_cache.insert(path, meta);
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize file metadata: {}", e);
                }
            }
        }
        
        tracing::info!(
            "Loaded {} file metadata entries for drive {}",
            drive_cache.len(),
            drive_id
        );
        
        Ok(())
    }

    /// Create a new document for a drive (owner only)
    pub async fn create_doc(&self, drive_id: DriveId) -> Result<NamespaceId> {
        // Check if already exists
        {
            let ns = self.namespaces.read().await;
            if let Some(namespace_id) = ns.get(&drive_id) {
                tracing::debug!("Doc already exists for drive {}", drive_id);
                return Ok(*namespace_id);
            }
        }

        // Generate a namespace ID from the drive ID
        let namespace_id = NamespaceId::from(*drive_id.as_bytes());

        tracing::info!("Created doc {} for drive {}", namespace_id, drive_id);

        // Store mapping
        let mut ns = self.namespaces.write().await;
        ns.insert(drive_id, namespace_id);

        // Initialize empty metadata cache and load from DB
        self.load_drive_metadata(&drive_id).await?;

        Ok(namespace_id)
    }

    /// Join an existing document via ticket (peer joining)
    pub async fn join_doc(&self, drive_id: DriveId, ticket: DocTicket) -> Result<NamespaceId> {
        let namespace_id = ticket.capability.id();

        tracing::info!("Joined doc {} for drive {}", namespace_id, drive_id);

        // Store mapping
        let mut ns = self.namespaces.write().await;
        ns.insert(drive_id, namespace_id);

        // Initialize metadata cache (will be populated via sync)
        let mut cache = self.metadata_cache.write().await;
        cache.insert(drive_id, HashMap::new());

        Ok(namespace_id)
    }

    /// Update file metadata in a drive's document (persists to DB)
    pub async fn set_file_metadata(&self, drive_id: &DriveId, meta: &FileMetadata) -> Result<()> {
        let drive_id_hex = hex::encode(drive_id.as_bytes());
        
        // Serialize and persist to database
        let data = serde_json::to_vec(meta)?;
        self.db.save_file_metadata(&drive_id_hex, &meta.path, &data)?;
        
        // Update in-memory cache
        let mut cache = self.metadata_cache.write().await;
        let drive_cache = cache
            .entry(*drive_id)
            .or_insert_with(HashMap::new);
        drive_cache.insert(meta.path.clone(), meta.clone());

        tracing::debug!("Saved metadata for {} in drive {}", meta.path, drive_id);

        Ok(())
    }

    /// Delete file metadata from a drive's document (persists to DB)
    pub async fn delete_file_metadata(&self, drive_id: &DriveId, path: &str) -> Result<()> {
        let drive_id_hex = hex::encode(drive_id.as_bytes());
        
        // Delete from database
        self.db.delete_file_metadata(&drive_id_hex, path)?;
        
        // Delete from in-memory cache
        let mut cache = self.metadata_cache.write().await;
        if let Some(drive_cache) = cache.get_mut(drive_id) {
            drive_cache.remove(path);
        }

        tracing::debug!("Deleted metadata for {} in drive {}", path, drive_id);

        Ok(())
    }

    /// Get all file metadata for a drive (from cache)
    pub async fn get_all_metadata(&self, drive_id: &DriveId) -> Result<Vec<FileMetadata>> {
        let cache = self.metadata_cache.read().await;
        
        match cache.get(drive_id) {
            Some(drive_cache) => Ok(drive_cache.values().cloned().collect()),
            None => {
                // Drive not in cache, try loading from DB
                drop(cache);
                self.load_drive_metadata(drive_id).await?;
                
                let cache = self.metadata_cache.read().await;
                Ok(cache
                    .get(drive_id)
                    .map(|c| c.values().cloned().collect())
                    .unwrap_or_default())
            }
        }
    }

    /// Get metadata for files in a specific directory
    pub async fn get_directory_metadata(&self, drive_id: &DriveId, dir_path: &str) -> Result<Vec<FileMetadata>> {
        let all_metadata = self.get_all_metadata(drive_id).await?;
        
        let normalized_dir = if dir_path.is_empty() || dir_path == "/" {
            String::new()
        } else {
            dir_path.trim_start_matches('/').to_string()
        };
        
        let result: Vec<FileMetadata> = all_metadata
            .into_iter()
            .filter(|meta| {
                let meta_path = meta.path.trim_start_matches('/');
                
                if normalized_dir.is_empty() {
                    // Root directory: only include files without path separator
                    !meta_path.contains('/')
                } else {
                    // Subdirectory: check if file is direct child
                    if let Some(remainder) = meta_path.strip_prefix(&normalized_dir) {
                        let remainder = remainder.trim_start_matches('/');
                        !remainder.is_empty() && !remainder.contains('/')
                    } else {
                        false
                    }
                }
            })
            .collect();
        
        Ok(result)
    }

    /// Generate a sharing ticket for a drive's document
    pub async fn get_ticket(&self, drive_id: &DriveId) -> Result<DocTicket> {
        let _namespace_id = {
            let ns = self.namespaces.read().await;
            *ns.get(drive_id)
                .ok_or_else(|| anyhow::anyhow!("Doc not found for drive {}", drive_id))?
        };

        // TODO: Implement full ticket generation with iroh-docs RPC
        Err(anyhow::anyhow!(
            "Ticket generation requires full iroh-docs integration (Phase 2b)"
        ))
    }

    /// Check if we have a document for a drive
    pub async fn has_doc(&self, drive_id: &DriveId) -> bool {
        let ns = self.namespaces.read().await;
        ns.contains_key(drive_id)
    }

    /// Get our author ID
    pub fn author_id(&self) -> AuthorId {
        // Convert placeholder to real AuthorId format
        AuthorId::from(self._author_id.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_metadata_doc_key() {
        let meta = FileMetadata::new(
            "docs/readme.md",
            "readme.md",
            false,
            1024,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(meta.doc_key(), b"file:docs/readme.md".to_vec());
    }

    #[test]
    fn test_file_metadata_serialization() {
        let meta = FileMetadata::new("test.txt", "test.txt", false, 512, "2024-01-01T00:00:00Z");
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: FileMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.path, parsed.path);
        assert_eq!(meta.size, parsed.size);
    }
}
