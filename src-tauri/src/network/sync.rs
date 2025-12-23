//! Sync Engine - Orchestrates real-time synchronization
//!
//! The SyncEngine coordinates:
//! - Metadata synchronization via DocsManager (CRDT)
//! - Event broadcasting via EventBroadcaster (gossip)
//! - Local change processing
//! - Remote event handling

use crate::core::{DriveEvent, DriveId, SharedDrive};
use crate::network::{DocsManager, EventBroadcaster};
use anyhow::Result;
use iroh_docs::DocTicket;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Coordinates metadata sync, event broadcasting, and file transfers
pub struct SyncEngine {
    /// Document manager for CRDT metadata sync
    docs_manager: Arc<DocsManager>,
    /// Event broadcaster for real-time gossip
    event_broadcaster: Arc<EventBroadcaster>,
    /// Internal event channel for coordination
    event_tx: broadcast::Sender<(DriveId, DriveEvent)>,
}

impl SyncEngine {
    /// Create a new SyncEngine
    pub fn new(
        docs_manager: Arc<DocsManager>,
        event_broadcaster: Arc<EventBroadcaster>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(512);

        tracing::info!("SyncEngine initialized");

        Self {
            docs_manager,
            event_broadcaster,
            event_tx,
        }
    }

    /// Initialize sync for an owned drive
    ///
    /// This sets up:
    /// 1. An iroh-doc for metadata sync
    /// 2. A gossip topic subscription for events
    pub async fn init_drive(&self, drive: &SharedDrive) -> Result<()> {
        let drive_id = drive.id;

        // 1. Create iroh-doc for this drive
        let _namespace_id = self.docs_manager.create_doc(drive_id).await?;

        // 2. Subscribe to gossip topic
        self.event_broadcaster.subscribe(drive_id).await?;

        tracing::info!("Sync initialized for owned drive: {}", drive_id);

        Ok(())
    }

    /// Join an existing drive via sharing ticket
    ///
    /// This sets up:
    /// 1. Import the iroh-doc from the ticket
    /// 2. Subscribe to the gossip topic
    pub async fn join_drive(&self, drive_id: DriveId, ticket: DocTicket) -> Result<()> {
        // 1. Import doc from ticket
        let _namespace_id = self.docs_manager.join_doc(drive_id, ticket).await?;

        // 2. Subscribe to gossip topic
        self.event_broadcaster.subscribe(drive_id).await?;

        tracing::info!("Sync initialized for joined drive: {}", drive_id);

        Ok(())
    }

    /// Stop syncing a drive
    pub async fn stop_sync(&self, drive_id: &DriveId) {
        self.event_broadcaster.unsubscribe(drive_id).await;
        tracing::info!("Sync stopped for drive: {}", drive_id);
    }

    /// Handle a local file change
    ///
    /// Called by the file watcher when a local change is detected.
    /// This will:
    /// 1. Update the iroh-doc metadata
    /// 2. Broadcast the event via gossip
    pub async fn on_local_change(&self, drive_id: &DriveId, event: DriveEvent) -> Result<()> {
        // Update metadata in docs based on event type
        match &event {
            DriveEvent::FileChanged {
                path,
                hash,
                size,
                modified_by: _,
                timestamp,
            } => {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let meta = crate::network::docs::FileMetadata {
                    name: file_name,
                    path: path.to_string_lossy().to_string(),
                    is_dir: false,
                    size: *size,
                    modified_at: timestamp.to_rfc3339(),
                    content_hash: Some(hash.clone()),
                    version: 1,
                };

                self.docs_manager.set_file_metadata(drive_id, &meta).await?;
            }
            DriveEvent::FileDeleted { path, .. } => {
                self.docs_manager
                    .delete_file_metadata(drive_id, &path.to_string_lossy())
                    .await?;
            }
            _ => {
                // Other events don't need metadata updates
            }
        }

        // Broadcast event via gossip
        self.event_broadcaster.broadcast(drive_id, event.clone()).await?;

        // Forward to internal channel
        let _ = self.event_tx.send((drive_id.clone(), event));

        Ok(())
    }

    /// Handle a remote event received via gossip
    ///
    /// Called when we receive an event from another peer.
    /// This will:
    /// 1. Update local state if needed
    /// 2. Forward to the internal event channel
    pub async fn on_remote_event(&self, drive_id: &DriveId, event: DriveEvent) -> Result<()> {
        // Update local metadata based on event
        match &event {
            DriveEvent::FileChanged {
                path,
                hash,
                size,
                modified_by: _,
                timestamp,
            } => {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let meta = crate::network::docs::FileMetadata {
                    name: file_name,
                    path: path.to_string_lossy().to_string(),
                    is_dir: false,
                    size: *size,
                    modified_at: timestamp.to_rfc3339(),
                    content_hash: Some(hash.clone()),
                    version: 1,
                };

                // Only update if we have a doc for this drive
                if self.docs_manager.has_doc(drive_id).await {
                    self.docs_manager.set_file_metadata(drive_id, &meta).await?;
                }
            }
            DriveEvent::FileDeleted { path, .. } => {
                if self.docs_manager.has_doc(drive_id).await {
                    self.docs_manager
                        .delete_file_metadata(drive_id, &path.to_string_lossy())
                        .await?;
                }
            }
            _ => {
                // Handle presence events, etc.
            }
        }

        // Forward to internal channel
        let _ = self.event_tx.send((drive_id.clone(), event));

        Ok(())
    }

    /// Get a receiver for internal sync events
    ///
    /// This can be used to listen for all events (local and remote).
    pub fn subscribe_events(&self) -> broadcast::Receiver<(DriveId, DriveEvent)> {
        self.event_tx.subscribe()
    }

    /// Check if a drive is being synced
    pub async fn is_syncing(&self, drive_id: &DriveId) -> bool {
        self.docs_manager.has_doc(drive_id).await
            && self.event_broadcaster.is_subscribed(drive_id).await
    }

    /// Get sync status for a drive
    pub async fn get_status(&self, drive_id: &DriveId) -> SyncStatus {
        let is_syncing = self.is_syncing(drive_id).await;
        let connected_peers = if is_syncing {
            // In Phase 2b, we'd query actual connected peers
            0
        } else {
            0
        };

        SyncStatus {
            is_syncing,
            connected_peers,
            last_sync: None,
        }
    }

    /// Get the docs manager for direct access
    pub fn docs_manager(&self) -> Arc<DocsManager> {
        self.docs_manager.clone()
    }

    /// Get the event broadcaster for direct access
    pub fn event_broadcaster(&self) -> Arc<EventBroadcaster> {
        self.event_broadcaster.clone()
    }
}

/// Status information for sync operations
#[derive(Clone, Debug, serde::Serialize)]
pub struct SyncStatus {
    /// Whether sync is active for this drive
    pub is_syncing: bool,
    /// Number of connected peers
    pub connected_peers: usize,
    /// Last successful sync timestamp (ISO 8601)
    pub last_sync: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests require mocking the DocsManager and EventBroadcaster
    // which would be added in integration tests

    #[test]
    fn test_sync_status_serialization() {
        let status = SyncStatus {
            is_syncing: true,
            connected_peers: 3,
            last_sync: Some("2024-01-01T00:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("is_syncing"));
        assert!(json.contains("connected_peers"));
    }
}
