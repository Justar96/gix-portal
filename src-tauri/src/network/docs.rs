//! Document synchronization via iroh-docs protocol
//!
//! Provides CRDT-based metadata synchronization for drives.
//! Each drive has its own iroh-docs document (namespace) that stores
//! file metadata and syncs automatically between peers.
//!
//! Note: This is the foundation for Phase 2. Full iroh-docs integration
//! requires additional setup with blobs and downloader components.

#![allow(dead_code)]

use crate::core::DriveId;
use anyhow::Result;
use iroh_docs::{AuthorId, DocTicket, NamespaceId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
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
/// This is a simplified implementation that stores metadata locally.
/// Full iroh-docs CRDT sync will be integrated when the blobs/downloader
/// infrastructure is set up.
pub struct DocsManager {
    /// Our author identity ID (placeholder until full integration)
    _author_id: PlaceholderAuthorId,
    /// Mapping from DriveId to document NamespaceId
    namespaces: RwLock<HashMap<DriveId, NamespaceId>>,
    /// Local metadata cache per drive
    metadata_cache: RwLock<HashMap<DriveId, HashMap<String, FileMetadata>>>,
    /// Data directory for persistent storage
    #[allow(dead_code)]
    data_dir: PathBuf,
}

impl DocsManager {
    /// Create a new DocsManager
    pub async fn new(data_dir: &std::path::Path) -> Result<Self> {
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
            _author_id: author_id,
            namespaces: RwLock::new(HashMap::new()),
            metadata_cache: RwLock::new(HashMap::new()),
            data_dir: data_dir.to_path_buf(),
        })
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

        // Initialize empty metadata cache
        let mut cache = self.metadata_cache.write().await;
        cache.insert(drive_id, HashMap::new());

        Ok(namespace_id)
    }

    /// Join an existing document via ticket (peer joining)
    ///
    /// Note: Full ticket parsing and peer sync requires iroh-docs RPC integration.
    /// This is a placeholder that extracts the namespace ID.
    pub async fn join_doc(&self, drive_id: DriveId, ticket: DocTicket) -> Result<NamespaceId> {
        let namespace_id = ticket.capability.id();

        tracing::info!("Joined doc {} for drive {}", namespace_id, drive_id);

        // Store mapping
        let mut ns = self.namespaces.write().await;
        ns.insert(drive_id, namespace_id);

        // Initialize empty metadata cache
        let mut cache = self.metadata_cache.write().await;
        cache.insert(drive_id, HashMap::new());

        Ok(namespace_id)
    }

    /// Update file metadata in a drive's document
    pub async fn set_file_metadata(&self, drive_id: &DriveId, meta: &FileMetadata) -> Result<()> {
        let mut cache = self.metadata_cache.write().await;
        let drive_cache = cache
            .get_mut(drive_id)
            .ok_or_else(|| anyhow::anyhow!("Doc not found for drive {}", drive_id))?;

        drive_cache.insert(meta.path.clone(), meta.clone());

        tracing::debug!("Updated metadata for {} in drive {}", meta.path, drive_id);

        Ok(())
    }

    /// Delete file metadata from a drive's document
    pub async fn delete_file_metadata(&self, drive_id: &DriveId, path: &str) -> Result<()> {
        let mut cache = self.metadata_cache.write().await;
        let drive_cache = cache
            .get_mut(drive_id)
            .ok_or_else(|| anyhow::anyhow!("Doc not found for drive {}", drive_id))?;

        drive_cache.remove(path);

        tracing::debug!("Deleted metadata for {} in drive {}", path, drive_id);

        Ok(())
    }

    /// Get all file metadata from a drive's document
    pub async fn get_all_metadata(&self, drive_id: &DriveId) -> Result<Vec<FileMetadata>> {
        let cache = self.metadata_cache.read().await;
        let drive_cache = cache
            .get(drive_id)
            .ok_or_else(|| anyhow::anyhow!("Doc not found for drive {}", drive_id))?;

        Ok(drive_cache.values().cloned().collect())
    }

    /// Generate a sharing ticket for a drive's document
    ///
    /// Note: Full ticket generation requires iroh-docs RPC integration.
    /// This is a placeholder that returns an error.
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

    #[tokio::test]
    async fn test_docs_manager_basic_ops() {
        let temp_dir = std::env::temp_dir().join("gix_test_docs");
        let manager = DocsManager::new(&temp_dir).await.unwrap();

        let drive_id = DriveId([1u8; 32]);

        // Create doc
        let _ns_id = manager.create_doc(drive_id).await.unwrap();
        assert!(manager.has_doc(&drive_id).await);

        // Set metadata
        let meta = FileMetadata::new("test.txt", "test.txt", false, 100, "2024-01-01T00:00:00Z");
        manager.set_file_metadata(&drive_id, &meta).await.unwrap();

        // Get metadata
        let all_meta = manager.get_all_metadata(&drive_id).await.unwrap();
        assert_eq!(all_meta.len(), 1);
        assert_eq!(all_meta[0].path, "test.txt");

        // Delete metadata
        manager
            .delete_file_metadata(&drive_id, "test.txt")
            .await
            .unwrap();
        let all_meta = manager.get_all_metadata(&drive_id).await.unwrap();
        assert_eq!(all_meta.len(), 0);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
