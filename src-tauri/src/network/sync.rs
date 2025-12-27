//! Sync Engine - Orchestrates real-time synchronization
//!
//! The SyncEngine coordinates:
//! - Metadata synchronization via DocsManager (CRDT)
//! - Event broadcasting via EventBroadcaster (gossip)
//! - Local change processing
//! - Remote event handling

#![allow(dead_code)]

use crate::core::{DriveEvent, DriveId, SharedDrive};
use crate::network::{DocsManager, EventBroadcaster};
use anyhow::Result;
use chrono::Utc;
use iroh_docs::DocTicket;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Coordinates metadata sync, event broadcasting, and file transfers
pub struct SyncEngine {
    /// Document manager for CRDT metadata sync
    docs_manager: Arc<DocsManager>,
    /// Event broadcaster for real-time gossip
    event_broadcaster: Arc<EventBroadcaster>,
    /// Internal event channel for coordination
    event_tx: broadcast::Sender<(DriveId, DriveEvent)>,
    /// Last error seen per drive for diagnostics
    last_error: RwLock<HashMap<DriveId, SyncErrorInfo>>,
}

impl SyncEngine {
    /// Create a new SyncEngine
    pub fn new(docs_manager: Arc<DocsManager>, event_broadcaster: Arc<EventBroadcaster>) -> Self {
        let (event_tx, _) = broadcast::channel(512);

        tracing::info!("SyncEngine initialized");

        Self {
            docs_manager,
            event_broadcaster,
            event_tx,
            last_error: RwLock::new(HashMap::new()),
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
        if let Err(err) = self.docs_manager.create_doc(drive_id).await {
            self.record_error(drive_id, format!("docs init failed: {}", err))
                .await;
            return Err(err);
        }

        // 2. Subscribe to gossip topic
        if let Err(err) = self.event_broadcaster.subscribe(drive_id).await {
            self.record_error(drive_id, format!("gossip subscribe failed: {}", err))
                .await;
            return Err(err);
        }

        tracing::info!("Sync initialized for owned drive: {}", drive_id);
        self.clear_error(&drive_id).await;

        Ok(())
    }

    /// Join an existing drive via sharing ticket
    ///
    /// This sets up:
    /// 1. Import the iroh-doc from the ticket
    /// 2. Wait for initial peer discovery
    /// 3. Subscribe to the gossip topic with discovered peers
    pub async fn join_drive(&self, drive_id: DriveId, ticket: DocTicket) -> Result<()> {
        // 1. Import doc from ticket
        if let Err(err) = self.docs_manager.join_doc(drive_id, ticket).await {
            self.record_error(drive_id, format!("docs join failed: {}", err))
                .await;
            return Err(err);
        }

        // 2. Wait briefly for docs to discover initial peers
        // This gives iroh-docs time to connect to peers from the ticket
        tracing::debug!("Waiting for peer discovery for drive {}", drive_id);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 3. Get discovered peers from docs to use as gossip bootstrap peers
        let bootstrap_peers =
            if let Ok(Some(peers)) = self.docs_manager.get_sync_peers(&drive_id).await {
                tracing::info!(
                    "Discovered {} peers for drive {} - using for gossip bootstrap",
                    peers.len(),
                    drive_id
                );
                // Convert PeerIdBytes to NodeId for gossip
                peers
                    .into_iter()
                    .filter_map(|peer_bytes| {
                        // PeerIdBytes is [u8; 32], NodeId can be constructed from it
                        iroh::NodeId::from_bytes(&peer_bytes).ok()
                    })
                    .collect()
            } else {
                tracing::warn!(
                    "No peers discovered for drive {} - gossip will rely on network discovery",
                    drive_id
                );
                vec![]
            };

        // 4. Subscribe to gossip topic with bootstrap peers
        if let Err(err) = self
            .event_broadcaster
            .subscribe_with_peers(drive_id, bootstrap_peers)
            .await
        {
            self.record_error(drive_id, format!("gossip subscribe failed: {}", err))
                .await;
            return Err(err);
        }

        tracing::info!("Sync initialized for joined drive: {}", drive_id);
        self.clear_error(&drive_id).await;

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

                if let Err(err) = self.docs_manager.set_file_metadata(drive_id, &meta).await {
                    self.record_error(*drive_id, format!("metadata update failed: {}", err))
                        .await;
                    return Err(err);
                }
            }
            DriveEvent::FileDeleted { path, .. } => {
                if let Err(err) = self
                    .docs_manager
                    .delete_file_metadata(drive_id, &path.to_string_lossy())
                    .await
                {
                    self.record_error(*drive_id, format!("metadata delete failed: {}", err))
                        .await;
                    return Err(err);
                }
            }
            _ => {
                // Other events don't need metadata updates
            }
        }

        // Broadcast event via gossip
        if let Err(err) = self
            .event_broadcaster
            .broadcast(drive_id, event.clone())
            .await
        {
            self.record_error(*drive_id, format!("gossip broadcast failed: {}", err))
                .await;
            return Err(err);
        }

        // Forward to internal channel
        let _ = self.event_tx.send((*drive_id, event));

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
                    if let Err(err) = self
                        .docs_manager
                        .set_file_metadata_cached(drive_id, &meta)
                        .await
                    {
                        self.record_error(*drive_id, format!("metadata update failed: {}", err))
                            .await;
                        return Err(err);
                    }
                }
            }
            DriveEvent::FileDeleted { path, .. } => {
                if self.docs_manager.has_doc(drive_id).await {
                    if let Err(err) = self
                        .docs_manager
                        .delete_file_metadata_cached(drive_id, &path.to_string_lossy())
                        .await
                    {
                        self.record_error(*drive_id, format!("metadata delete failed: {}", err))
                            .await;
                        return Err(err);
                    }
                }
            }
            _ => {
                // Handle presence events, etc.
            }
        }

        // Forward to internal channel
        let _ = self.event_tx.send((*drive_id, event));

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
            self.docs_manager
                .get_sync_peers(drive_id)
                .await
                .ok()
                .flatten()
                .map(|peers| peers.len())
                .unwrap_or(0)
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

    /// Get sync diagnostics for a drive
    pub async fn get_diagnostics(&self, drive_id: &DriveId) -> SyncDiagnostics {
        let has_doc = self.docs_manager.has_doc(drive_id).await;
        let gossip_subscribed = self.event_broadcaster.is_subscribed(drive_id).await;
        let namespace = self.docs_manager.namespace_id(drive_id).await;
        let doc_peers = self
            .docs_manager
            .get_sync_peers(drive_id)
            .await
            .ok()
            .flatten()
            .map(|peers| peers.len());
        let last_error = self.get_last_error(drive_id).await;

        SyncDiagnostics {
            is_syncing: has_doc && gossip_subscribed,
            has_doc,
            gossip_subscribed,
            doc_namespace: namespace.map(|id| id.to_string()),
            doc_peers,
            last_error,
        }
    }

    async fn record_error(&self, drive_id: DriveId, message: String) {
        let mut errors = self.last_error.write().await;
        errors.insert(
            drive_id,
            SyncErrorInfo {
                message,
                timestamp: Utc::now().to_rfc3339(),
            },
        );
    }

    async fn clear_error(&self, drive_id: &DriveId) {
        let mut errors = self.last_error.write().await;
        errors.remove(drive_id);
    }

    async fn get_last_error(&self, drive_id: &DriveId) -> Option<SyncErrorInfo> {
        let errors = self.last_error.read().await;
        errors.get(drive_id).cloned()
    }
}

/// Diagnostics for sync setup and connectivity
#[derive(Clone, Debug, serde::Serialize)]
pub struct SyncDiagnostics {
    /// Whether sync is active for this drive
    pub is_syncing: bool,
    /// Whether a docs replica exists for this drive
    pub has_doc: bool,
    /// Whether gossip is subscribed for this drive
    pub gossip_subscribed: bool,
    /// Document namespace ID (hex)
    pub doc_namespace: Option<String>,
    /// Number of peers reported by docs sync
    pub doc_peers: Option<usize>,
    /// Most recent error
    pub last_error: Option<SyncErrorInfo>,
}

/// Last error info for diagnostics
#[derive(Clone, Debug, serde::Serialize)]
pub struct SyncErrorInfo {
    pub message: String,
    pub timestamp: String,
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

    #[test]
    fn test_sync_status_default_values() {
        let status = SyncStatus {
            is_syncing: false,
            connected_peers: 0,
            last_sync: None,
        };

        assert!(!status.is_syncing);
        assert_eq!(status.connected_peers, 0);
        assert!(status.last_sync.is_none());
    }

    #[test]
    fn test_sync_status_with_last_sync() {
        let status = SyncStatus {
            is_syncing: true,
            connected_peers: 5,
            last_sync: Some("2024-12-25T10:30:00Z".to_string()),
        };

        assert!(status.is_syncing);
        assert_eq!(status.connected_peers, 5);
        assert_eq!(status.last_sync.as_deref(), Some("2024-12-25T10:30:00Z"));
    }

    #[test]
    fn test_sync_status_clone() {
        let status = SyncStatus {
            is_syncing: true,
            connected_peers: 10,
            last_sync: Some("2024-01-01T00:00:00Z".to_string()),
        };

        let cloned = status.clone();

        assert_eq!(status.is_syncing, cloned.is_syncing);
        assert_eq!(status.connected_peers, cloned.connected_peers);
        assert_eq!(status.last_sync, cloned.last_sync);
    }

    #[test]
    fn test_sync_status_debug() {
        let status = SyncStatus {
            is_syncing: true,
            connected_peers: 2,
            last_sync: None,
        };

        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("SyncStatus"));
        assert!(debug_str.contains("is_syncing"));
        assert!(debug_str.contains("connected_peers"));
    }

    #[test]
    fn test_sync_status_json_structure() {
        let status = SyncStatus {
            is_syncing: false,
            connected_peers: 0,
            last_sync: None,
        };

        let json: serde_json::Value = serde_json::to_value(&status).unwrap();

        assert!(json.is_object());
        assert!(json.get("is_syncing").is_some());
        assert!(json.get("connected_peers").is_some());
        assert!(json.get("last_sync").is_some());
    }

    #[test]
    fn test_sync_diagnostics_serialization() {
        let diagnostics = SyncDiagnostics {
            is_syncing: true,
            has_doc: true,
            gossip_subscribed: true,
            doc_namespace: Some("abc123".to_string()),
            doc_peers: Some(2),
            last_error: Some(SyncErrorInfo {
                message: "test error".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            }),
        };

        let json = serde_json::to_string(&diagnostics).unwrap();
        assert!(json.contains("is_syncing"));
        assert!(json.contains("doc_namespace"));
        assert!(json.contains("last_error"));
    }
}
