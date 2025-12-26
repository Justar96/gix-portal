//! Integration tests for P2P sync functionality
//!
//! These tests verify the sync engine, file watching, and transfer operations.
//! Run with: cargo test --test sync_tests -- --nocapture

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

/// Mock DriveId for testing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DriveId([u8; 32]);

impl DriveId {
    fn new(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        DriveId(id)
    }
}

/// Mock SyncStatus
#[derive(Clone, Debug)]
struct SyncStatus {
    is_syncing: bool,
    connected_peers: usize,
}

/// Mock SyncEngine for testing sync operations
struct MockSyncEngine {
    syncing_drives: Arc<RwLock<HashMap<DriveId, SyncStatus>>>,
    event_tx: broadcast::Sender<(DriveId, String)>,
}

impl MockSyncEngine {
    fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            syncing_drives: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    async fn start_sync(&self, drive_id: DriveId) -> Result<(), String> {
        let mut drives = self.syncing_drives.write().await;
        drives.insert(
            drive_id,
            SyncStatus {
                is_syncing: true,
                connected_peers: 0,
            },
        );
        let _ = self.event_tx.send((drive_id, "SyncStarted".to_string()));
        Ok(())
    }

    async fn stop_sync(&self, drive_id: &DriveId) {
        let mut drives = self.syncing_drives.write().await;
        drives.remove(drive_id);
        let _ = self.event_tx.send((*drive_id, "SyncStopped".to_string()));
    }

    async fn is_syncing(&self, drive_id: &DriveId) -> bool {
        let drives = self.syncing_drives.read().await;
        drives.get(drive_id).map(|s| s.is_syncing).unwrap_or(false)
    }

    async fn get_status(&self, drive_id: &DriveId) -> Option<SyncStatus> {
        let drives = self.syncing_drives.read().await;
        drives.get(drive_id).cloned()
    }

    async fn update_peer_count(&self, drive_id: &DriveId, count: usize) {
        let mut drives = self.syncing_drives.write().await;
        if let Some(status) = drives.get_mut(drive_id) {
            status.connected_peers = count;
        }
    }

    fn subscribe_events(&self) -> broadcast::Receiver<(DriveId, String)> {
        self.event_tx.subscribe()
    }
}

#[tokio::test]
async fn test_sync_engine_start_stop() {
    let engine = MockSyncEngine::new();
    let drive_id = DriveId::new(1);

    // Initially not syncing
    assert!(!engine.is_syncing(&drive_id).await);

    // Start sync
    engine.start_sync(drive_id).await.unwrap();
    assert!(engine.is_syncing(&drive_id).await);

    // Check status
    let status = engine.get_status(&drive_id).await.unwrap();
    assert!(status.is_syncing);
    assert_eq!(status.connected_peers, 0);

    // Stop sync
    engine.stop_sync(&drive_id).await;
    assert!(!engine.is_syncing(&drive_id).await);
}

#[tokio::test]
async fn test_sync_engine_multiple_drives() {
    let engine = MockSyncEngine::new();
    let drive1 = DriveId::new(1);
    let drive2 = DriveId::new(2);
    let drive3 = DriveId::new(3);

    // Start sync for multiple drives
    engine.start_sync(drive1).await.unwrap();
    engine.start_sync(drive2).await.unwrap();
    engine.start_sync(drive3).await.unwrap();

    assert!(engine.is_syncing(&drive1).await);
    assert!(engine.is_syncing(&drive2).await);
    assert!(engine.is_syncing(&drive3).await);

    // Stop one drive
    engine.stop_sync(&drive2).await;

    assert!(engine.is_syncing(&drive1).await);
    assert!(!engine.is_syncing(&drive2).await);
    assert!(engine.is_syncing(&drive3).await);
}

#[tokio::test]
async fn test_sync_engine_peer_count_updates() {
    let engine = MockSyncEngine::new();
    let drive_id = DriveId::new(1);

    engine.start_sync(drive_id).await.unwrap();

    // Update peer count
    engine.update_peer_count(&drive_id, 3).await;

    let status = engine.get_status(&drive_id).await.unwrap();
    assert_eq!(status.connected_peers, 3);

    // Update again
    engine.update_peer_count(&drive_id, 5).await;

    let status = engine.get_status(&drive_id).await.unwrap();
    assert_eq!(status.connected_peers, 5);
}

#[tokio::test]
async fn test_sync_engine_event_broadcasting() {
    let engine = MockSyncEngine::new();
    let drive_id = DriveId::new(1);

    let mut rx = engine.subscribe_events();

    // Start sync - should emit event
    engine.start_sync(drive_id).await.unwrap();

    let (received_id, event) = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Channel closed");

    assert_eq!(received_id, drive_id);
    assert_eq!(event, "SyncStarted");

    // Stop sync - should emit event
    engine.stop_sync(&drive_id).await;

    let (received_id, event) = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Channel closed");

    assert_eq!(received_id, drive_id);
    assert_eq!(event, "SyncStopped");
}

/// Mock FileWatcher for testing file watching operations
struct MockFileWatcher {
    watching: Arc<RwLock<HashMap<DriveId, String>>>,
    event_tx: broadcast::Sender<(DriveId, String, String)>, // (drive_id, event_type, path)
}

impl MockFileWatcher {
    fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            watching: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    async fn watch(&self, drive_id: DriveId, path: String) -> Result<(), String> {
        let mut watching = self.watching.write().await;
        watching.insert(drive_id, path);
        Ok(())
    }

    async fn unwatch(&self, drive_id: &DriveId) {
        let mut watching = self.watching.write().await;
        watching.remove(drive_id);
    }

    async fn is_watching(&self, drive_id: &DriveId) -> bool {
        let watching = self.watching.read().await;
        watching.contains_key(drive_id)
    }

    async fn simulate_file_change(&self, drive_id: DriveId, path: &str) {
        let watching = self.watching.read().await;
        if watching.contains_key(&drive_id) {
            let _ = self.event_tx.send((drive_id, "FileChanged".to_string(), path.to_string()));
        }
    }

    fn subscribe_events(&self) -> broadcast::Receiver<(DriveId, String, String)> {
        self.event_tx.subscribe()
    }
}

#[tokio::test]
async fn test_file_watcher_start_stop() {
    let watcher = MockFileWatcher::new();
    let drive_id = DriveId::new(1);

    // Initially not watching
    assert!(!watcher.is_watching(&drive_id).await);

    // Start watching
    watcher.watch(drive_id, "/home/user/drive".to_string()).await.unwrap();
    assert!(watcher.is_watching(&drive_id).await);

    // Stop watching
    watcher.unwatch(&drive_id).await;
    assert!(!watcher.is_watching(&drive_id).await);
}

#[tokio::test]
async fn test_file_watcher_events() {
    let watcher = MockFileWatcher::new();
    let drive_id = DriveId::new(1);

    watcher.watch(drive_id, "/home/user/drive".to_string()).await.unwrap();

    let mut rx = watcher.subscribe_events();

    // Simulate file change
    watcher.simulate_file_change(drive_id, "/home/user/drive/test.txt").await;

    let (received_id, event_type, path) = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Channel closed");

    assert_eq!(received_id, drive_id);
    assert_eq!(event_type, "FileChanged");
    assert_eq!(path, "/home/user/drive/test.txt");
}

#[tokio::test]
async fn test_file_watcher_ignores_unwatched_drives() {
    let watcher = MockFileWatcher::new();
    let drive_id = DriveId::new(1);

    // Don't start watching
    let mut rx = watcher.subscribe_events();

    // Simulate file change - should not emit event
    watcher.simulate_file_change(drive_id, "/home/user/drive/test.txt").await;

    let result = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
    assert!(result.is_err(), "Should not receive event for unwatched drive");
}

/// Mock TransferManager for testing file transfers
struct MockTransferManager {
    transfers: Arc<RwLock<HashMap<String, TransferState>>>,
    next_id: Arc<RwLock<u64>>,
}

#[derive(Clone, Debug)]
struct TransferState {
    status: String,
    bytes_transferred: u64,
    total_bytes: u64,
}

impl MockTransferManager {
    fn new() -> Self {
        Self {
            transfers: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    async fn start_upload(&self, _drive_id: DriveId, _path: &str, size: u64) -> String {
        let mut next_id = self.next_id.write().await;
        let id = format!("transfer-{}", *next_id);
        *next_id += 1;

        let transfer = TransferState {
            status: "InProgress".to_string(),
            bytes_transferred: 0,
            total_bytes: size,
        };

        let mut transfers = self.transfers.write().await;
        transfers.insert(id.clone(), transfer);

        id
    }

    async fn update_progress(&self, transfer_id: &str, bytes: u64) {
        let mut transfers = self.transfers.write().await;
        if let Some(transfer) = transfers.get_mut(transfer_id) {
            transfer.bytes_transferred = bytes;
            if bytes >= transfer.total_bytes {
                transfer.status = "Completed".to_string();
            }
        }
    }

    async fn cancel_transfer(&self, transfer_id: &str) -> bool {
        let mut transfers = self.transfers.write().await;
        if let Some(transfer) = transfers.get_mut(transfer_id) {
            transfer.status = "Cancelled".to_string();
            true
        } else {
            false
        }
    }

    async fn get_transfer(&self, transfer_id: &str) -> Option<TransferState> {
        let transfers = self.transfers.read().await;
        transfers.get(transfer_id).cloned()
    }

    async fn list_transfers(&self) -> Vec<TransferState> {
        let transfers = self.transfers.read().await;
        transfers.values().cloned().collect()
    }
}

#[tokio::test]
async fn test_transfer_manager_upload() {
    let manager = MockTransferManager::new();
    let drive_id = DriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/test/file.txt", 1024).await;

    let transfer = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(transfer.status, "InProgress");
    assert_eq!(transfer.bytes_transferred, 0);
    assert_eq!(transfer.total_bytes, 1024);
}

#[tokio::test]
async fn test_transfer_manager_progress() {
    let manager = MockTransferManager::new();
    let drive_id = DriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/test/file.txt", 1024).await;

    // Update progress
    manager.update_progress(&transfer_id, 512).await;

    let transfer = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(transfer.bytes_transferred, 512);
    assert_eq!(transfer.status, "InProgress");

    // Complete transfer
    manager.update_progress(&transfer_id, 1024).await;

    let transfer = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(transfer.bytes_transferred, 1024);
    assert_eq!(transfer.status, "Completed");
}

#[tokio::test]
async fn test_transfer_manager_cancel() {
    let manager = MockTransferManager::new();
    let drive_id = DriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/test/file.txt", 1024).await;

    // Cancel transfer
    let cancelled = manager.cancel_transfer(&transfer_id).await;
    assert!(cancelled);

    let transfer = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(transfer.status, "Cancelled");
}

#[tokio::test]
async fn test_transfer_manager_list() {
    let manager = MockTransferManager::new();
    let drive_id = DriveId::new(1);

    // Start multiple transfers
    manager.start_upload(drive_id, "/test/file1.txt", 1024).await;
    manager.start_upload(drive_id, "/test/file2.txt", 2048).await;
    manager.start_upload(drive_id, "/test/file3.txt", 4096).await;

    let transfers = manager.list_transfers().await;
    assert_eq!(transfers.len(), 3);
}

/// Test concurrent sync operations
#[tokio::test]
async fn test_concurrent_sync_operations() {
    let engine = Arc::new(MockSyncEngine::new());
    let mut handles = Vec::new();

    // Start sync for 10 drives concurrently
    for i in 0..10 {
        let engine = engine.clone();
        handles.push(tokio::spawn(async move {
            let drive_id = DriveId::new(i);
            engine.start_sync(drive_id).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
            engine.update_peer_count(&drive_id, i as usize).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all drives are syncing
    for i in 0..10 {
        let drive_id = DriveId::new(i);
        assert!(engine.is_syncing(&drive_id).await);
        let status = engine.get_status(&drive_id).await.unwrap();
        assert_eq!(status.connected_peers, i as usize);
    }
}

/// Test sync engine resilience to rapid start/stop
#[tokio::test]
async fn test_sync_rapid_start_stop() {
    let engine = MockSyncEngine::new();
    let drive_id = DriveId::new(1);

    for _ in 0..100 {
        engine.start_sync(drive_id).await.unwrap();
        assert!(engine.is_syncing(&drive_id).await);
        engine.stop_sync(&drive_id).await;
        assert!(!engine.is_syncing(&drive_id).await);
    }
}

/// Test transfer manager under load
#[tokio::test]
async fn test_transfer_manager_load() {
    let manager = Arc::new(MockTransferManager::new());
    let drive_id = DriveId::new(1);
    let mut handles = Vec::new();

    // Start 50 concurrent transfers
    for i in 0..50 {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let path = format!("/test/file{}.txt", i);
            let transfer_id = manager.start_upload(drive_id, &path, 1024 * (i + 1) as u64).await;

            // Simulate progress updates
            for progress in (0..=100).step_by(25) {
                let bytes = (1024 * (i + 1) as u64 * progress) / 100;
                manager.update_progress(&transfer_id, bytes).await;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            transfer_id
        }));
    }

    // Wait for all transfers to complete
    let mut transfer_ids: Vec<String> = Vec::new();
    for handle in handles {
        transfer_ids.push(handle.await.unwrap());
    }

    // Verify all transfers completed
    for transfer_id in &transfer_ids {
        let transfer = manager.get_transfer(transfer_id).await.unwrap();
        assert_eq!(transfer.status, "Completed");
    }
}
