//! File Transfer Manager - Handles bidirectional file sync via iroh-blobs
//!
//! This module provides:
//! - Upload: Local files → iroh-blobs store → available to peers
//! - Download: Peer blobs → iroh-blobs store → local files
//! - Progress tracking for transfers
//! - Atomic writes using temp files

#![allow(dead_code)]

use crate::core::{DriveEvent, DriveId};
use crate::crypto::NodeId;
use anyhow::{Context, Result};
use chrono::Utc;
use iroh::Endpoint;
use iroh_blobs::{
    net_protocol::Blobs,
    store::{fs::Store as BlobStore, Map, MapEntry, Store as StoreExt},
    Hash, BlobFormat,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Transfer state for tracking active transfers
#[derive(Clone, Debug, Serialize)]
pub struct TransferState {
    /// Unique transfer ID
    pub id: String,
    /// Drive this transfer belongs to
    pub drive_id: String,
    /// File path (relative to drive root)
    pub path: String,
    /// Transfer direction
    pub direction: TransferDirection,
    /// Current state
    pub status: TransferStatus,
    /// Bytes transferred so far
    pub bytes_transferred: u64,
    /// Total bytes to transfer
    pub total_bytes: u64,
    /// BLAKE3 hash of the content
    pub hash: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Transfer direction
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Transfer status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransferStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Progress event for transfers
#[derive(Clone, Debug, Serialize)]
pub struct TransferProgress {
    pub transfer_id: String,
    pub drive_id: String,
    pub path: String,
    pub direction: TransferDirection,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub status: TransferStatus,
}

/// Manages file transfers using iroh-blobs
pub struct FileTransferManager {
    /// The iroh-blobs protocol handler
    blobs: Arc<Blobs<BlobStore>>,
    /// Our node ID for attribution
    node_id: NodeId,
    /// Active transfers
    transfers: Arc<RwLock<HashMap<String, TransferState>>>,
    /// Progress event channel
    progress_tx: broadcast::Sender<TransferProgress>,
    /// Drive event channel (for sync events)
    event_tx: broadcast::Sender<(DriveId, DriveEvent)>,
}

impl FileTransferManager {
    /// Create a new file transfer manager
    ///
    /// # Arguments
    /// * `endpoint` - The Iroh endpoint for P2P connections
    /// * `data_dir` - Directory to store blob data
    /// * `node_id` - Our node ID for event attribution
    pub async fn new(
        endpoint: &Endpoint,
        data_dir: &Path,
        node_id: NodeId,
    ) -> Result<Self> {
        let blobs_dir = data_dir.join("blobs");
        std::fs::create_dir_all(&blobs_dir)?;

        // Create persistent blob store
        let blobs = Blobs::persistent(&blobs_dir)
            .await
            .context("Failed to create blob store")?
            .build(endpoint);

        let (progress_tx, _) = broadcast::channel(256);
        let (event_tx, _) = broadcast::channel(256);

        tracing::info!("FileTransferManager initialized at {:?}", blobs_dir);

        Ok(Self {
            blobs: Arc::new(blobs),
            node_id,
            transfers: Arc::new(RwLock::new(HashMap::new())),
            progress_tx,
            event_tx,
        })
    }

    /// Subscribe to transfer progress events
    pub fn subscribe_progress(&self) -> broadcast::Receiver<TransferProgress> {
        self.progress_tx.subscribe()
    }

    /// Subscribe to drive events (file sync completed, etc.)
    pub fn subscribe_events(&self) -> broadcast::Receiver<(DriveId, DriveEvent)> {
        self.event_tx.subscribe()
    }

    /// Upload a file to the blob store
    ///
    /// This imports a local file into iroh-blobs, making it available to peers.
    /// Returns the hash of the uploaded content.
    pub async fn upload_file(
        &self,
        drive_id: &DriveId,
        local_path: &Path,
        relative_path: &Path,
    ) -> Result<Hash> {
        let transfer_id = generate_transfer_id();
        let drive_id_str = hex::encode(drive_id.as_bytes());

        // Get file size for progress tracking
        let metadata = tokio::fs::metadata(local_path)
            .await
            .context("Failed to get file metadata")?;
        let total_bytes = metadata.len();

        // Create transfer state
        let state = TransferState {
            id: transfer_id.clone(),
            drive_id: drive_id_str.clone(),
            path: relative_path.to_string_lossy().to_string(),
            direction: TransferDirection::Upload,
            status: TransferStatus::InProgress,
            bytes_transferred: 0,
            total_bytes,
            hash: None,
            error: None,
        };

        // Store transfer state
        self.transfers.write().await.insert(transfer_id.clone(), state);

        // Emit initial progress
        self.emit_progress(&transfer_id).await;

        // Import file into blob store
        let outcome = self.import_file(local_path).await?;

        // Update transfer state with hash
        {
            let mut transfers = self.transfers.write().await;
            if let Some(state) = transfers.get_mut(&transfer_id) {
                state.status = TransferStatus::Completed;
                state.bytes_transferred = total_bytes;
                state.hash = Some(outcome.to_hex().to_string());
            }
        }

        // Emit completion progress
        self.emit_progress(&transfer_id).await;

        // Emit sync complete event
        let event = DriveEvent::SyncComplete {
            path: relative_path.to_path_buf(),
            hash: outcome.to_hex().to_string(),
        };
        let _ = self.event_tx.send((*drive_id, event));

        tracing::info!(
            "Uploaded file {} -> hash {}",
            local_path.display(),
            outcome.to_hex()
        );

        Ok(outcome)
    }

    /// Download a file from the blob store to local filesystem
    ///
    /// This exports a blob from the store to a local file path.
    /// Uses atomic writes (temp file → rename) to prevent partial writes.
    pub async fn download_file(
        &self,
        drive_id: &DriveId,
        hash: Hash,
        local_path: &Path,
        relative_path: &Path,
    ) -> Result<()> {
        let transfer_id = generate_transfer_id();
        let drive_id_str = hex::encode(drive_id.as_bytes());

        // Get blob size for progress tracking
        let store = self.blobs.store();
        let entry = store
            .get(&hash)
            .await?
            .context("Blob not found in store")?;
        let total_bytes = entry.size().value();

        // Create transfer state
        let state = TransferState {
            id: transfer_id.clone(),
            drive_id: drive_id_str.clone(),
            path: relative_path.to_string_lossy().to_string(),
            direction: TransferDirection::Download,
            status: TransferStatus::InProgress,
            bytes_transferred: 0,
            total_bytes,
            hash: Some(hash.to_hex().to_string()),
            error: None,
        };

        self.transfers.write().await.insert(transfer_id.clone(), state);
        self.emit_progress(&transfer_id).await;

        // Create parent directories if needed
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Use atomic write: write to temp file, then rename
        let temp_path = local_path.with_extension("tmp.download");

        // Export blob to temp file
        match self.export_file(hash, &temp_path).await {
            Ok(()) => {
                // Atomic rename
                tokio::fs::rename(&temp_path, local_path).await?;

                // Update transfer state
                {
                    let mut transfers = self.transfers.write().await;
                    if let Some(state) = transfers.get_mut(&transfer_id) {
                        state.status = TransferStatus::Completed;
                        state.bytes_transferred = total_bytes;
                    }
                }

                self.emit_progress(&transfer_id).await;

                // Emit file changed event
                let event = DriveEvent::FileChanged {
                    path: relative_path.to_path_buf(),
                    hash: hash.to_hex().to_string(),
                    size: total_bytes,
                    modified_by: self.node_id,
                    timestamp: Utc::now(),
                };
                let _ = self.event_tx.send((*drive_id, event));

                tracing::info!(
                    "Downloaded hash {} -> {}",
                    hash.to_hex(),
                    local_path.display()
                );

                Ok(())
            }
            Err(e) => {
                // Clean up temp file on error
                let _ = tokio::fs::remove_file(&temp_path).await;

                // Update transfer state
                {
                    let mut transfers = self.transfers.write().await;
                    if let Some(state) = transfers.get_mut(&transfer_id) {
                        state.status = TransferStatus::Failed;
                        state.error = Some(e.to_string());
                    }
                }

                self.emit_progress(&transfer_id).await;
                Err(e)
            }
        }
    }

    /// Download a blob from a remote peer
    ///
    /// Fetches the blob from the specified peer and adds it to local store.
    /// Note: This is a simplified implementation. Full peer-to-peer download
    /// would require more complex coordination with the downloader.
    pub async fn download_from_peer(
        &self,
        drive_id: &DriveId,
        hash: Hash,
        _peer_node_id: iroh::NodeId,
        local_path: &Path,
        relative_path: &Path,
    ) -> Result<()> {
        let transfer_id = generate_transfer_id();
        let drive_id_str = hex::encode(drive_id.as_bytes());

        // Create transfer state (size unknown until we start downloading)
        let state = TransferState {
            id: transfer_id.clone(),
            drive_id: drive_id_str,
            path: relative_path.to_string_lossy().to_string(),
            direction: TransferDirection::Download,
            status: TransferStatus::InProgress,
            bytes_transferred: 0,
            total_bytes: 0, // Unknown until we get the blob
            hash: Some(hash.to_hex().to_string()),
            error: None,
        };

        self.transfers.write().await.insert(transfer_id.clone(), state);
        self.emit_progress(&transfer_id).await;

        // Check if blob already exists in local store
        let store = self.blobs.store();
        if let Some(entry) = store.get(&hash).await? {
            if entry.is_complete() {
                // Blob already available locally, just export it
                return self.download_file(drive_id, hash, local_path, relative_path).await;
            }
        }

        // TODO: Implement full peer-to-peer download using downloader
        // For now, we require the blob to be available locally
        // The full implementation would use:
        // let request = DownloadRequest::new(HashAndFormat { hash, format: BlobFormat::Raw }, vec![peer_node_id.into()]);
        // let handle = downloader.queue(request).await;
        // handle.await?;

        anyhow::bail!(
            "Blob {} not found locally. Peer-to-peer download not yet fully implemented.",
            hash.to_hex()
        )
    }

    /// Import a file into the blob store (internal helper)
    ///
    /// Uses iroh's import_file which computes the hash internally,
    /// avoiding the need to read the entire file into memory.
    async fn import_file(&self, path: &Path) -> Result<Hash> {
        let store = self.blobs.store();
        let path_buf = path.to_path_buf();

        use iroh_blobs::store::ImportMode;
        use iroh_blobs::util::progress::IgnoreProgressSender;

        // iroh's import_file handles both storage and hash computation
        // without loading the entire file into memory
        let (tag, _size) = store
            .import_file(
                path_buf,
                ImportMode::Copy,
                BlobFormat::Raw,
                IgnoreProgressSender::default(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to import file: {}", e))?;

        // Get hash from the returned tag - no redundant file read needed
        Ok(*tag.hash())
    }

    /// Export a blob to a file (internal helper)
    ///
    /// Uses streaming to avoid loading the entire blob into memory.
    /// Reads in 64KB chunks and writes directly to disk.
    async fn export_file(&self, hash: Hash, path: &Path) -> Result<()> {
        use iroh_io::AsyncSliceReader;
        use tokio::io::AsyncWriteExt;

        let store = self.blobs.store();
        let entry = store.get(&hash).await?.context("Blob not found")?;
        let total_size = entry.size().value();

        // Stream chunks to file instead of loading entire blob into memory
        let mut reader = entry.data_reader();
        let mut file = tokio::fs::File::create(path).await?;
        let mut written = 0u64;
        const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

        while written < total_size {
            let remaining = total_size - written;
            let chunk_size = std::cmp::min(CHUNK_SIZE as u64, remaining) as usize;

            // Read chunk from blob at current offset
            let data = reader.read_at(written, chunk_size).await?;
            if data.is_empty() {
                break;
            }

            file.write_all(&data).await?;
            written += data.len() as u64;
        }

        file.flush().await?;
        Ok(())
    }

    /// Emit progress event for a transfer
    async fn emit_progress(&self, transfer_id: &str) {
        let transfers = self.transfers.read().await;
        if let Some(state) = transfers.get(transfer_id) {
            let progress = TransferProgress {
                transfer_id: state.id.clone(),
                drive_id: state.drive_id.clone(),
                path: state.path.clone(),
                direction: state.direction.clone(),
                bytes_transferred: state.bytes_transferred,
                total_bytes: state.total_bytes,
                status: state.status.clone(),
            };
            let _ = self.progress_tx.send(progress);
        }
    }

    /// Get all active transfers
    pub async fn list_transfers(&self) -> Vec<TransferState> {
        self.transfers.read().await.values().cloned().collect()
    }

    /// Get transfer by ID
    pub async fn get_transfer(&self, transfer_id: &str) -> Option<TransferState> {
        self.transfers.read().await.get(transfer_id).cloned()
    }

    /// Cancel a transfer
    pub async fn cancel_transfer(&self, transfer_id: &str) -> Result<()> {
        let mut transfers = self.transfers.write().await;
        if let Some(state) = transfers.get_mut(transfer_id) {
            if state.status == TransferStatus::InProgress || state.status == TransferStatus::Pending {
                state.status = TransferStatus::Cancelled;
                tracing::info!("Cancelled transfer: {}", transfer_id);
            }
        }
        Ok(())
    }

    /// Clean up completed/failed transfers older than the specified duration
    pub async fn cleanup_old_transfers(&self, _max_age: std::time::Duration) {
        // For now, just clear completed transfers
        // In a real implementation, we'd track timestamps
        let mut transfers = self.transfers.write().await;
        transfers.retain(|_, state| {
            state.status == TransferStatus::InProgress || state.status == TransferStatus::Pending
        });
    }

    /// Get the underlying blob store for advanced operations
    pub fn store(&self) -> &BlobStore {
        self.blobs.store()
    }

    /// Get blob hash for a file path (if it exists in store)
    ///
    /// Uses streaming BLAKE3 hasher to compute hash without loading
    /// the entire file into memory.
    pub async fn get_blob_hash(&self, local_path: &Path) -> Result<Option<Hash>> {
        use tokio::io::AsyncReadExt;

        // Stream file through BLAKE3 hasher in 64KB chunks
        let file = tokio::fs::File::open(local_path).await?;
        let mut reader = tokio::io::BufReader::with_capacity(64 * 1024, file);
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0u8; 64 * 1024];

        loop {
            let bytes_read = reader.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        // Convert BLAKE3 hash to iroh_blobs::Hash
        let blake3_hash = hasher.finalize();
        let hash = Hash::from_bytes(*blake3_hash.as_bytes());

        // Check if it exists in store
        let store = self.blobs.store();
        if store.get(&hash).await?.is_some() {
            Ok(Some(hash))
        } else {
            Ok(None)
        }
    }
}

/// Generate a unique transfer ID
fn generate_transfer_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("xfer_{:x}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_transfer_id() {
        let id1 = generate_transfer_id();
        let id2 = generate_transfer_id();

        assert!(id1.starts_with("xfer_"));
        assert!(id2.starts_with("xfer_"));
        // IDs should be different (unless called at exact same nanosecond)
    }

    #[test]
    fn test_transfer_state_serialization() {
        let state = TransferState {
            id: "xfer_123".to_string(),
            drive_id: "abc123".to_string(),
            path: "test/file.txt".to_string(),
            direction: TransferDirection::Upload,
            status: TransferStatus::Completed,
            bytes_transferred: 1024,
            total_bytes: 1024,
            hash: Some("deadbeef".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("xfer_123"));
        assert!(json.contains("Upload"));
        assert!(json.contains("Completed"));
    }
}
