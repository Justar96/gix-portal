//! Document synchronization via iroh-docs protocol
//!
//! Provides CRDT-based metadata synchronization for drives.
//! Each drive has its own iroh-docs document (namespace) that stores
//! file metadata and syncs automatically between peers.
//!
//! Metadata is persisted to database and synced via gossip.

#![allow(dead_code)]

use crate::core::DriveId;
use crate::crypto::Permission;
use crate::storage::Database;
use anyhow::{anyhow, Result};
use futures_lite::StreamExt;
use iroh_blobs::store::Map;
use iroh_blobs::{net_protocol::Blobs, store::fs::Store as BlobStore, Hash};
use iroh_docs::protocol::Docs;
use iroh_docs::rpc::client::docs::{Doc, MemClient, ShareMode};
use iroh_docs::rpc::proto::{Request as DocsRequest, Response as DocsResponse};
use iroh_docs::rpc::AddrInfoOptions;
use iroh_docs::store::Query;
use iroh_docs::{AuthorId, DocTicket, Entry, NamespaceId, PeerIdBytes};
use iroh_gossip::net::Gossip;
use iroh_io::AsyncSliceReader;
use quic_rpc::transport::flume::FlumeConnector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const DOC_KEY_PREFIX: &str = "file:";
type MemDoc = Doc<FlumeConnector<DocsResponse, DocsRequest>>;

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
    pub fn with_hash(
        path: &str,
        name: &str,
        is_dir: bool,
        size: u64,
        modified_at: &str,
        hash: String,
    ) -> Self {
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
        format!("{}{}", DOC_KEY_PREFIX, self.path).into_bytes()
    }
}

/// Manages document metadata for drives
///
/// Stores metadata in database for persistence and in memory for fast access.
/// Syncs with peers via gossip protocol.
pub struct DocsManager {
    /// Database for persistence
    db: Arc<Database>,
    /// Docs protocol handle (kept alive for background tasks)
    docs: Docs<BlobStore>,
    /// Docs RPC client
    docs_client: MemClient,
    /// Our author identity ID
    author_id: AuthorId,
    /// Blobs protocol for reading metadata bytes
    blobs: Arc<Blobs<BlobStore>>,
    /// Mapping from DriveId to document NamespaceId
    namespaces: RwLock<HashMap<DriveId, NamespaceId>>,
    /// Open doc handles per drive
    docs_by_drive: RwLock<HashMap<DriveId, MemDoc>>,
    /// In-memory metadata cache per drive (for fast lookups)
    metadata_cache: RwLock<HashMap<DriveId, HashMap<String, FileMetadata>>>,
    /// Data directory for persistent storage
    #[allow(dead_code)]
    data_dir: PathBuf,
}

impl DocsManager {
    /// Create a new DocsManager with database persistence
    pub async fn new(
        data_dir: &std::path::Path,
        db: Arc<Database>,
        blobs: Arc<Blobs<BlobStore>>,
        gossip: Arc<Gossip>,
    ) -> Result<Self> {
        // Create directories for docs storage
        let docs_dir = data_dir.join("docs");
        std::fs::create_dir_all(&docs_dir)?;

        let docs = Docs::persistent(docs_dir.clone())
            .spawn(blobs.as_ref(), gossip.as_ref())
            .await?;

        if let Err(err) = blobs.add_protected(docs.protect_cb()) {
            tracing::warn!(error = %err, "Failed to register docs protect callback");
        }

        let docs_client = docs.client().clone();
        let author_id = docs_client.authors().default().await?;

        let mut namespaces = HashMap::new();
        for (drive_id, namespace) in db.list_doc_namespaces()? {
            namespaces.insert(DriveId(drive_id), NamespaceId::from(&namespace));
        }

        tracing::info!("DocsManager initialized with author: {}", author_id);

        Ok(Self {
            db,
            docs,
            docs_client,
            author_id,
            blobs,
            namespaces: RwLock::new(namespaces),
            docs_by_drive: RwLock::new(HashMap::new()),
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
        if let Some(doc) = self.get_or_open_doc(&drive_id).await? {
            tracing::debug!("Doc already exists for drive {}", drive_id);
            return Ok(doc.id());
        }

        let doc = self.docs_client.create().await?;
        let namespace_id = doc.id();

        self.store_namespace_mapping(drive_id, namespace_id).await?;
        self.docs_by_drive
            .write()
            .await
            .insert(drive_id, doc.clone());

        self.load_drive_metadata(&drive_id).await?;
        self.sync_cache_to_doc(&drive_id, &doc).await?;

        tracing::info!("Created doc {} for drive {}", namespace_id, drive_id);

        Ok(namespace_id)
    }

    /// Join an existing document via ticket (peer joining)
    pub async fn join_doc(&self, drive_id: DriveId, ticket: DocTicket) -> Result<NamespaceId> {
        let doc = self.docs_client.import(ticket).await?;
        let namespace_id = doc.id();

        self.store_namespace_mapping(drive_id, namespace_id).await?;
        self.docs_by_drive.write().await.insert(drive_id, doc);

        // Initialize metadata cache from DB (remote entries will update on demand)
        self.load_drive_metadata(&drive_id).await?;

        tracing::info!("Joined doc {} for drive {}", namespace_id, drive_id);

        Ok(namespace_id)
    }

    /// Update file metadata in a drive's document (persists to DB)
    pub async fn set_file_metadata(&self, drive_id: &DriveId, meta: &FileMetadata) -> Result<()> {
        self.set_file_metadata_cached(drive_id, meta).await?;

        let Some(doc) = self.get_or_open_doc(drive_id).await? else {
            return Ok(());
        };

        let data = serde_json::to_vec(meta)?;
        doc.set_bytes(self.author_id, meta.doc_key(), data).await?;

        tracing::debug!("Saved metadata for {} in drive {}", meta.path, drive_id);

        Ok(())
    }

    /// Delete file metadata from a drive's document (persists to DB)
    pub async fn delete_file_metadata(&self, drive_id: &DriveId, path: &str) -> Result<()> {
        self.delete_file_metadata_cached(drive_id, path).await?;

        let Some(doc) = self.get_or_open_doc(drive_id).await? else {
            return Ok(());
        };

        doc.del(self.author_id, format!("{}{}", DOC_KEY_PREFIX, path))
            .await?;

        tracing::debug!("Deleted metadata for {} in drive {}", path, drive_id);

        Ok(())
    }

    /// Update metadata cache and DB without touching the docs replica
    pub async fn set_file_metadata_cached(
        &self,
        drive_id: &DriveId,
        meta: &FileMetadata,
    ) -> Result<()> {
        let drive_id_hex = hex::encode(drive_id.as_bytes());

        // Serialize and persist to database
        let data = serde_json::to_vec(meta)?;
        self.db
            .save_file_metadata(&drive_id_hex, &meta.path, &data)?;

        // Update in-memory cache
        let mut cache = self.metadata_cache.write().await;
        let drive_cache = cache.entry(*drive_id).or_insert_with(HashMap::new);
        drive_cache.insert(meta.path.clone(), meta.clone());

        Ok(())
    }

    /// Delete metadata cache and DB without touching the docs replica
    pub async fn delete_file_metadata_cached(&self, drive_id: &DriveId, path: &str) -> Result<()> {
        let drive_id_hex = hex::encode(drive_id.as_bytes());

        // Delete from database
        self.db.delete_file_metadata(&drive_id_hex, path)?;

        // Delete from in-memory cache
        let mut cache = self.metadata_cache.write().await;
        if let Some(drive_cache) = cache.get_mut(drive_id) {
            drive_cache.remove(path);
        }

        Ok(())
    }

    /// Get all file metadata for a drive (from cache)
    pub async fn get_all_metadata(&self, drive_id: &DriveId) -> Result<Vec<FileMetadata>> {
        if let Err(err) = self.refresh_from_doc(drive_id).await {
            tracing::debug!(error = %err, drive_id = %drive_id, "Failed to refresh metadata from doc");
        }

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
    pub async fn get_directory_metadata(
        &self,
        drive_id: &DriveId,
        dir_path: &str,
    ) -> Result<Vec<FileMetadata>> {
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
                } else if let Some(remainder) = meta_path.strip_prefix(&normalized_dir) {
                    let remainder = remainder.trim_start_matches('/');
                    !remainder.is_empty() && !remainder.contains('/')
                } else {
                    false
                }
            })
            .collect();

        Ok(result)
    }

    /// Generate a sharing ticket for a drive's document
    pub async fn get_ticket(
        &self,
        drive_id: &DriveId,
        permission: Permission,
    ) -> Result<DocTicket> {
        let doc = self
            .get_or_open_doc(drive_id)
            .await?
            .ok_or_else(|| anyhow!("Doc not found for drive {}", drive_id))?;

        let share_mode = match permission {
            Permission::Read => ShareMode::Read,
            _ => ShareMode::Write,
        };

        let ticket = doc
            .share(share_mode, AddrInfoOptions::RelayAndAddresses)
            .await?;

        Ok(ticket)
    }

    /// Check if we have a document for a drive
    pub async fn has_doc(&self, drive_id: &DriveId) -> bool {
        let ns = self.namespaces.read().await;
        ns.contains_key(drive_id)
    }

    /// Get the document namespace for a drive
    pub async fn namespace_id(&self, drive_id: &DriveId) -> Option<NamespaceId> {
        let ns = self.namespaces.read().await;
        ns.get(drive_id).copied()
    }

    /// Get sync peers for a drive document
    pub async fn get_sync_peers(&self, drive_id: &DriveId) -> Result<Option<Vec<PeerIdBytes>>> {
        let Some(doc) = self.get_or_open_doc(drive_id).await? else {
            return Ok(None);
        };

        doc.get_sync_peers().await
    }

    /// Get our author ID
    pub fn author_id(&self) -> AuthorId {
        self.author_id
    }

    async fn store_namespace_mapping(
        &self,
        drive_id: DriveId,
        namespace_id: NamespaceId,
    ) -> Result<()> {
        self.db
            .save_doc_namespace(drive_id.as_bytes(), namespace_id.as_bytes())?;
        let mut ns = self.namespaces.write().await;
        ns.insert(drive_id, namespace_id);
        Ok(())
    }

    async fn get_or_open_doc(&self, drive_id: &DriveId) -> Result<Option<MemDoc>> {
        if let Some(doc) = self.docs_by_drive.read().await.get(drive_id).cloned() {
            return Ok(Some(doc));
        }

        let namespace_id = {
            let ns = self.namespaces.read().await;
            ns.get(drive_id).copied()
        };

        let Some(namespace_id) = namespace_id else {
            return Ok(None);
        };

        let doc = self.docs_client.open(namespace_id).await?;
        if let Some(doc) = doc.clone() {
            self.docs_by_drive.write().await.insert(*drive_id, doc);
        }

        Ok(doc)
    }

    async fn sync_cache_to_doc(&self, drive_id: &DriveId, doc: &MemDoc) -> Result<()> {
        let cache = self.metadata_cache.read().await;
        let drive_cache = match cache.get(drive_id) {
            Some(cache) => cache,
            None => return Ok(()),
        };

        for meta in drive_cache.values() {
            let data = serde_json::to_vec(meta)?;
            if let Err(err) = doc.set_bytes(self.author_id, meta.doc_key(), data).await {
                tracing::warn!(
                    error = %err,
                    drive_id = %drive_id,
                    path = %meta.path,
                    "Failed to seed metadata into doc"
                );
            }
        }

        Ok(())
    }

    async fn refresh_from_doc(&self, drive_id: &DriveId) -> Result<()> {
        let Some(doc) = self.get_or_open_doc(drive_id).await? else {
            return Ok(());
        };

        let query = Query::single_latest_per_key()
            .key_prefix(DOC_KEY_PREFIX.as_bytes())
            .include_empty()
            .build();

        let mut stream = doc.get_many(query).await?;
        let mut updates: Vec<(String, Option<FileMetadata>)> = Vec::new();

        while let Some(entry) = stream.next().await {
            let entry = entry?;
            let Some(path) = Self::path_from_key(entry.key()) else {
                continue;
            };

            if entry.content_len() == 0 || entry.content_hash() == Hash::EMPTY {
                updates.push((path, None));
                continue;
            }

            let Some(bytes) = self.read_entry_bytes(&entry).await? else {
                continue;
            };

            match serde_json::from_slice::<FileMetadata>(&bytes) {
                Ok(mut meta) => {
                    if meta.path != path {
                        tracing::warn!(
                            drive_id = %drive_id,
                            key_path = %path,
                            meta_path = %meta.path,
                            "Metadata path mismatch; using doc key path"
                        );
                        meta.path = path.clone();
                    }
                    updates.push((path, Some(meta)));
                }
                Err(err) => {
                    tracing::warn!(error = %err, drive_id = %drive_id, "Failed to decode doc metadata");
                }
            }
        }

        if updates.is_empty() {
            return Ok(());
        }

        let drive_id_hex = hex::encode(drive_id.as_bytes());
        let mut cache = self.metadata_cache.write().await;
        let drive_cache = cache.entry(*drive_id).or_insert_with(HashMap::new);

        for (path, meta) in updates {
            match meta {
                Some(meta) => {
                    let data = serde_json::to_vec(&meta)?;
                    self.db.save_file_metadata(&drive_id_hex, &path, &data)?;
                    drive_cache.insert(path, meta);
                }
                None => {
                    self.db.delete_file_metadata(&drive_id_hex, &path)?;
                    drive_cache.remove(&path);
                }
            }
        }

        Ok(())
    }

    async fn read_entry_bytes(&self, entry: &Entry) -> Result<Option<Vec<u8>>> {
        let len = usize::try_from(entry.content_len()).ok();
        let Some(len) = len else {
            tracing::warn!("Entry length too large to read into memory");
            return Ok(None);
        };

        let hash = entry.content_hash();
        let Some(map_entry) = self.blobs.store().get(&hash).await? else {
            return Ok(None);
        };

        if !map_entry.is_complete() {
            return Ok(None);
        }

        let mut reader = map_entry.data_reader();
        let bytes = reader.read_exact_at(0, len).await?;
        Ok(Some(bytes.to_vec()))
    }

    fn path_from_key(key: &[u8]) -> Option<String> {
        let key_str = std::str::from_utf8(key).ok()?;
        key_str
            .strip_prefix(DOC_KEY_PREFIX)
            .map(|path| path.to_string())
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
