//! File Transfer Integration Tests
//!
//! Tests for file upload, download, progress tracking, and transfer management.
//! These tests verify the file transfer system works correctly including:
//! - Upload and download operations
//! - Progress tracking and callbacks
//! - Concurrent transfers
//! - Error handling and cancellation
//! - Hash verification
//!
//! Run with: cargo test --test transfer_tests -- --nocapture

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

// ===========================================================================
// Test Infrastructure
// ===========================================================================

/// Mock DriveId
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MockDriveId([u8; 32]);

impl MockDriveId {
    fn new(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        MockDriveId(id)
    }
}

/// Transfer direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransferDirection {
    Upload,
    Download,
}

/// Transfer status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransferStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

/// Transfer state
#[derive(Clone, Debug)]
struct TransferState {
    id: String,
    drive_id: MockDriveId,
    path: PathBuf,
    direction: TransferDirection,
    status: TransferStatus,
    bytes_transferred: u64,
    total_bytes: u64,
    hash: Option<String>,
    created_at: Instant,
    completed_at: Option<Instant>,
}

impl TransferState {
    fn is_complete(&self) -> bool {
        matches!(self.status, TransferStatus::Completed | TransferStatus::Cancelled)
    }
}

/// Progress event for UI updates
#[derive(Clone, Debug)]
struct TransferProgress {
    bytes_transferred: u64,
    total_bytes: u64,
}

/// Mock File Transfer Manager
struct MockTransferManager {
    transfers: Arc<RwLock<HashMap<String, TransferState>>>,
    next_id: Arc<AtomicU64>,
    progress_tx: broadcast::Sender<TransferProgress>,
    simulated_speed: u64, // bytes per tick
}

impl MockTransferManager {
    fn new() -> Self {
        let (progress_tx, _) = broadcast::channel(256);
        Self {
            transfers: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            progress_tx,
            simulated_speed: 1024 * 1024, // 1 MB per tick
        }
    }

    fn with_speed(mut self, speed: u64) -> Self {
        self.simulated_speed = speed;
        self
    }

    fn subscribe_progress(&self) -> broadcast::Receiver<TransferProgress> {
        self.progress_tx.subscribe()
    }

    async fn start_upload(
        &self,
        drive_id: MockDriveId,
        path: &str,
        total_bytes: u64,
    ) -> String {
        let id = format!("upload-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        
        let state = TransferState {
            id: id.clone(),
            drive_id,
            path: PathBuf::from(path),
            direction: TransferDirection::Upload,
            status: TransferStatus::Pending,
            bytes_transferred: 0,
            total_bytes,
            hash: None,
            created_at: Instant::now(),
            completed_at: None,
        };

        self.transfers.write().await.insert(id.clone(), state);
        id
    }

    async fn start_download(
        &self,
        drive_id: MockDriveId,
        path: &str,
        hash: &str,
        total_bytes: u64,
    ) -> String {
        let id = format!("download-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        
        let state = TransferState {
            id: id.clone(),
            drive_id,
            path: PathBuf::from(path),
            direction: TransferDirection::Download,
            status: TransferStatus::Pending,
            bytes_transferred: 0,
            total_bytes,
            hash: Some(hash.to_string()),
            created_at: Instant::now(),
            completed_at: None,
        };

        self.transfers.write().await.insert(id.clone(), state);
        id
    }

    async fn process_transfer(&self, transfer_id: &str) -> Result<String, String> {
        // Mark as in progress
        {
            let mut transfers = self.transfers.write().await;
            if let Some(state) = transfers.get_mut(transfer_id) {
                state.status = TransferStatus::InProgress;
            } else {
                return Err("Transfer not found".to_string());
            }
        }

        // Simulate transfer progress
        loop {
            let mut transfers = self.transfers.write().await;
            let state = transfers.get_mut(transfer_id).ok_or("Transfer not found")?;

            if matches!(state.status, TransferStatus::Cancelled) {
                return Err("Transfer cancelled".to_string());
            }

            let remaining = state.total_bytes - state.bytes_transferred;
            let chunk = remaining.min(self.simulated_speed);
            state.bytes_transferred += chunk;

            // Emit progress event
            let progress = TransferProgress {
                bytes_transferred: state.bytes_transferred,
                total_bytes: state.total_bytes,
            };
            let _ = self.progress_tx.send(progress);

            if state.bytes_transferred >= state.total_bytes {
                state.status = TransferStatus::Completed;
                state.completed_at = Some(Instant::now());
                
                // Generate hash if upload
                if state.direction == TransferDirection::Upload {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(state.path.to_string_lossy().as_bytes());
                    hasher.update(&state.total_bytes.to_le_bytes());
                    state.hash = Some(hasher.finalize().to_hex().to_string());
                }

                return Ok(state.hash.clone().unwrap_or_default());
            }

            drop(transfers);
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
    }

    async fn cancel_transfer(&self, transfer_id: &str) -> Result<(), String> {
        let mut transfers = self.transfers.write().await;
        if let Some(state) = transfers.get_mut(transfer_id) {
            if state.is_complete() {
                return Err("Transfer already completed".to_string());
            }
            state.status = TransferStatus::Cancelled;
            state.completed_at = Some(Instant::now());
            Ok(())
        } else {
            Err("Transfer not found".to_string())
        }
    }

    async fn get_transfer(&self, transfer_id: &str) -> Option<TransferState> {
        self.transfers.read().await.get(transfer_id).cloned()
    }

    async fn list_transfers(&self, drive_id: Option<MockDriveId>) -> Vec<TransferState> {
        let transfers = self.transfers.read().await;
        transfers
            .values()
            .filter(|t| drive_id.map(|d| t.drive_id == d).unwrap_or(true))
            .cloned()
            .collect()
    }

    async fn list_active_transfers(&self) -> Vec<TransferState> {
        let transfers = self.transfers.read().await;
        transfers
            .values()
            .filter(|t| !t.is_complete())
            .cloned()
            .collect()
    }

    async fn clear_completed(&self) {
        let mut transfers = self.transfers.write().await;
        transfers.retain(|_, t| !t.is_complete());
    }
}

// ===========================================================================
// Upload Tests
// ===========================================================================

#[tokio::test]
async fn test_upload_basic() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/test/file.txt", 1024).await;

    // Check initial state
    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Pending);
    assert_eq!(state.bytes_transferred, 0);
    assert_eq!(state.total_bytes, 1024);

    // Process transfer
    let hash = manager.process_transfer(&transfer_id).await.unwrap();
    assert!(!hash.is_empty());

    // Check completed state
    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Completed);
    assert_eq!(state.bytes_transferred, 1024);
    assert!(state.hash.is_some());
}

#[tokio::test]
async fn test_upload_large_file() {
    let manager = MockTransferManager::new().with_speed(10 * 1024 * 1024); // 10 MB/tick
    let drive_id = MockDriveId::new(1);

    let file_size = 100 * 1024 * 1024; // 100 MB
    let transfer_id = manager.start_upload(drive_id, "/large/video.mp4", file_size).await;

    let start = Instant::now();
    let hash = manager.process_transfer(&transfer_id).await.unwrap();
    let elapsed = start.elapsed();

    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Completed);
    assert!(!hash.is_empty());

    println!(
        "Large file upload: {} MB in {:?}",
        file_size / (1024 * 1024),
        elapsed
    );
}

#[tokio::test]
async fn test_upload_zero_byte_file() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/empty.txt", 0).await;
    let hash = manager.process_transfer(&transfer_id).await.unwrap();

    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Completed);
    assert_eq!(state.bytes_transferred, 0);
    assert!(!hash.is_empty());
}

#[tokio::test]
async fn test_upload_progress_tracking() {
    let manager = MockTransferManager::new().with_speed(256); // Small chunks for more progress events
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/progress/test.bin", 1024).await;
    
    let mut rx = manager.subscribe_progress();
    let received_progress = Arc::new(AtomicUsize::new(0));
    let progress_clone = received_progress.clone();

    // Spawn progress receiver
    let receiver = tokio::spawn(async move {
        while let Ok(_) = rx.recv().await {
            progress_clone.fetch_add(1, Ordering::Relaxed);
        }
    });

    manager.process_transfer(&transfer_id).await.unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;
    receiver.abort();

    let progress_count = received_progress.load(Ordering::Relaxed);
    assert!(progress_count > 1, "Should have received multiple progress events");
    println!("Received {} progress events", progress_count);
}

// ===========================================================================
// Download Tests
// ===========================================================================

#[tokio::test]
async fn test_download_basic() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);
    let expected_hash = "abc123def456";

    let transfer_id = manager
        .start_download(drive_id, "/remote/file.txt", expected_hash, 2048)
        .await;

    let hash = manager.process_transfer(&transfer_id).await.unwrap();

    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Completed);
    assert_eq!(state.bytes_transferred, 2048);
    assert_eq!(hash, expected_hash);
}

#[tokio::test]
async fn test_download_multiple_files() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let files = vec![
        ("/file1.txt", "hash1", 1024u64),
        ("/file2.txt", "hash2", 2048u64),
        ("/file3.txt", "hash3", 4096u64),
    ];

    let mut transfer_ids = Vec::new();
    for (path, hash, size) in &files {
        let id = manager.start_download(drive_id, path, hash, *size).await;
        transfer_ids.push(id);
    }

    for id in &transfer_ids {
        manager.process_transfer(id).await.unwrap();
    }

    for (i, id) in transfer_ids.iter().enumerate() {
        let state = manager.get_transfer(id).await.unwrap();
        assert_eq!(state.status, TransferStatus::Completed);
        assert_eq!(state.total_bytes, files[i].2);
    }
}

// ===========================================================================
// Cancellation Tests
// ===========================================================================

#[tokio::test]
async fn test_cancel_pending_transfer() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/cancel/test.txt", 10 * 1024 * 1024).await;

    // Cancel before processing
    manager.cancel_transfer(&transfer_id).await.unwrap();

    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Cancelled);
}

#[tokio::test]
async fn test_cancel_in_progress_transfer() {
    let manager = Arc::new(MockTransferManager::new().with_speed(1024)); // Slow transfer
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/slow/file.bin", 1024 * 1024).await;

    let manager_clone = manager.clone();
    let id_clone = transfer_id.clone();

    // Start processing in background
    let process_handle = tokio::spawn(async move {
        manager_clone.process_transfer(&id_clone).await
    });

    // Wait a bit then cancel
    tokio::time::sleep(Duration::from_millis(10)).await;
    manager.cancel_transfer(&transfer_id).await.unwrap();

    // Process should return error
    let result = process_handle.await.unwrap();
    assert!(result.is_err());

    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert_eq!(state.status, TransferStatus::Cancelled);
}

#[tokio::test]
async fn test_cancel_completed_transfer_fails() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/completed.txt", 1024).await;
    manager.process_transfer(&transfer_id).await.unwrap();

    // Try to cancel completed transfer
    let result = manager.cancel_transfer(&transfer_id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already completed"));
}

// ===========================================================================
// Concurrent Transfer Tests
// ===========================================================================

#[tokio::test]
async fn test_concurrent_uploads() {
    let manager = Arc::new(MockTransferManager::new());
    let drive_id = MockDriveId::new(1);

    let mut handles = Vec::new();
    const TRANSFER_COUNT: usize = 20;

    for i in 0..TRANSFER_COUNT {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let path = format!("/concurrent/file{}.txt", i);
            let size = (i + 1) as u64 * 1024;
            
            let transfer_id = manager.start_upload(drive_id, &path, size).await;
            manager.process_transfer(&transfer_id).await
        }));
    }

    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(success_count, TRANSFER_COUNT);

    let all_transfers = manager.list_transfers(Some(drive_id)).await;
    assert_eq!(all_transfers.len(), TRANSFER_COUNT);

    for transfer in &all_transfers {
        assert_eq!(transfer.status, TransferStatus::Completed);
    }
}

#[tokio::test]
async fn test_concurrent_uploads_and_downloads() {
    let manager = Arc::new(MockTransferManager::new());
    let drive_id = MockDriveId::new(1);

    let mut handles = Vec::new();

    // Start uploads
    for i in 0..10 {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let path = format!("/upload{}.txt", i);
            let transfer_id = manager.start_upload(drive_id, &path, 1024).await;
            manager.process_transfer(&transfer_id).await
        }));
    }

    // Start downloads
    for i in 0..10 {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let path = format!("/download{}.txt", i);
            let hash = format!("hash{}", i);
            let transfer_id = manager.start_download(drive_id, &path, &hash, 2048).await;
            manager.process_transfer(&transfer_id).await
        }));
    }

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let all_transfers = manager.list_transfers(None).await;
    assert_eq!(all_transfers.len(), 20);

    let uploads: Vec<_> = all_transfers.iter().filter(|t| t.direction == TransferDirection::Upload).collect();
    let downloads: Vec<_> = all_transfers.iter().filter(|t| t.direction == TransferDirection::Download).collect();

    assert_eq!(uploads.len(), 10);
    assert_eq!(downloads.len(), 10);
}

// ===========================================================================
// Transfer Listing Tests
// ===========================================================================

#[tokio::test]
async fn test_list_transfers_by_drive() {
    let manager = MockTransferManager::new();
    let drive1 = MockDriveId::new(1);
    let drive2 = MockDriveId::new(2);

    // Add transfers to different drives
    manager.start_upload(drive1, "/drive1/file1.txt", 1024).await;
    manager.start_upload(drive1, "/drive1/file2.txt", 1024).await;
    manager.start_upload(drive2, "/drive2/file1.txt", 1024).await;

    let drive1_transfers = manager.list_transfers(Some(drive1)).await;
    let drive2_transfers = manager.list_transfers(Some(drive2)).await;
    let all_transfers = manager.list_transfers(None).await;

    assert_eq!(drive1_transfers.len(), 2);
    assert_eq!(drive2_transfers.len(), 1);
    assert_eq!(all_transfers.len(), 3);
}

#[tokio::test]
async fn test_list_active_transfers() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let id1 = manager.start_upload(drive_id, "/active1.txt", 1024).await;
    let id2 = manager.start_upload(drive_id, "/active2.txt", 1024).await;
    let id3 = manager.start_upload(drive_id, "/active3.txt", 1024).await;

    // Complete one
    manager.process_transfer(&id1).await.unwrap();
    // Cancel one
    manager.cancel_transfer(&id2).await.unwrap();

    let active = manager.list_active_transfers().await;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, id3);
}

#[tokio::test]
async fn test_clear_completed_transfers() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let id1 = manager.start_upload(drive_id, "/clear1.txt", 1024).await;
    let id2 = manager.start_upload(drive_id, "/clear2.txt", 1024).await;
    let id3 = manager.start_upload(drive_id, "/clear3.txt", 1024).await;

    // Complete some
    manager.process_transfer(&id1).await.unwrap();
    manager.process_transfer(&id2).await.unwrap();

    // Clear completed
    manager.clear_completed().await;

    let remaining = manager.list_transfers(None).await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, id3);
}

// ===========================================================================
// Progress Calculation Tests
// ===========================================================================

#[tokio::test]
async fn test_progress_percent_calculation() {
    let manager = MockTransferManager::new().with_speed(256);
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/progress.txt", 1024).await;

    let mut rx = manager.subscribe_progress();
    let progress_values = Arc::new(RwLock::new(Vec::new()));
    let values_clone = progress_values.clone();

    let receiver = tokio::spawn(async move {
        while let Ok(progress) = rx.recv().await {
            let percent = (progress.bytes_transferred as f64 / progress.total_bytes as f64) * 100.0;
            values_clone.write().await.push(percent);
        }
    });

    manager.process_transfer(&transfer_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    receiver.abort();

    let values = progress_values.read().await;
    assert!(!values.is_empty());
    
    // Verify progress increases monotonically
    for window in values.windows(2) {
        assert!(window[1] >= window[0], "Progress should not decrease");
    }
    
    // Last value should be 100%
    if let Some(&last) = values.last() {
        assert!((last - 100.0).abs() < 0.1, "Final progress should be 100%");
    }
}

// ===========================================================================
// Error Handling Tests
// ===========================================================================

#[tokio::test]
async fn test_process_nonexistent_transfer() {
    let manager = MockTransferManager::new();

    let result = manager.process_transfer("nonexistent-id").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[tokio::test]
async fn test_cancel_nonexistent_transfer() {
    let manager = MockTransferManager::new();

    let result = manager.cancel_transfer("nonexistent-id").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[tokio::test]
async fn test_get_nonexistent_transfer() {
    let manager = MockTransferManager::new();

    let result = manager.get_transfer("nonexistent-id").await;
    assert!(result.is_none());
}

// ===========================================================================
// Transfer State Tests
// ===========================================================================

#[tokio::test]
async fn test_transfer_state_is_complete() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let id1 = manager.start_upload(drive_id, "/state1.txt", 1024).await;
    let id2 = manager.start_upload(drive_id, "/state2.txt", 1024).await;
    let id3 = manager.start_upload(drive_id, "/state3.txt", 1024).await;

    // Pending - not complete
    assert!(!manager.get_transfer(&id1).await.unwrap().is_complete());

    // Process id1 - should be complete
    manager.process_transfer(&id1).await.unwrap();
    assert!(manager.get_transfer(&id1).await.unwrap().is_complete());

    // Cancel id2 - should be complete
    manager.cancel_transfer(&id2).await.unwrap();
    assert!(manager.get_transfer(&id2).await.unwrap().is_complete());

    // id3 still pending
    assert!(!manager.get_transfer(&id3).await.unwrap().is_complete());
}

#[tokio::test]
async fn test_transfer_timestamps() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let before = Instant::now();
    let transfer_id = manager.start_upload(drive_id, "/timestamp.txt", 1024).await;
    
    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert!(state.created_at >= before);
    assert!(state.completed_at.is_none());

    manager.process_transfer(&transfer_id).await.unwrap();

    let state = manager.get_transfer(&transfer_id).await.unwrap();
    assert!(state.completed_at.is_some());
    assert!(state.completed_at.unwrap() >= state.created_at);
}

// ===========================================================================
// Performance Tests
// ===========================================================================

#[tokio::test]
async fn test_transfer_throughput() {
    let manager = MockTransferManager::new().with_speed(100 * 1024 * 1024); // 100 MB/tick
    let drive_id = MockDriveId::new(1);

    let file_size = 1024 * 1024 * 1024; // 1 GB
    let transfer_id = manager.start_upload(drive_id, "/huge.bin", file_size).await;

    let start = Instant::now();
    manager.process_transfer(&transfer_id).await.unwrap();
    let elapsed = start.elapsed();

    let throughput_mb = (file_size as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

    println!(
        "Transfer throughput: {:.0} MB/sec ({} GB in {:?})",
        throughput_mb,
        file_size / (1024 * 1024 * 1024),
        elapsed
    );
}

#[tokio::test]
async fn test_many_small_transfers() {
    let manager = Arc::new(MockTransferManager::new());
    let drive_id = MockDriveId::new(1);

    let start = Instant::now();
    const TRANSFER_COUNT: usize = 100;

    let mut handles = Vec::new();
    for i in 0..TRANSFER_COUNT {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let path = format!("/small/file{}.txt", i);
            let transfer_id = manager.start_upload(drive_id, &path, 1024).await;
            manager.process_transfer(&transfer_id).await
        }));
    }

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let elapsed = start.elapsed();
    let transfers_per_sec = TRANSFER_COUNT as f64 / elapsed.as_secs_f64();

    println!(
        "Small transfers: {:.0} transfers/sec ({} transfers in {:?})",
        transfers_per_sec, TRANSFER_COUNT, elapsed
    );

    assert!(
        transfers_per_sec > 100.0,
        "Should complete at least 100 transfers/sec"
    );
}

// ===========================================================================
// Hash Verification Tests
// ===========================================================================

#[tokio::test]
async fn test_upload_generates_hash() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let transfer_id = manager.start_upload(drive_id, "/hashtest.txt", 4096).await;
    let hash = manager.process_transfer(&transfer_id).await.unwrap();

    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 64); // BLAKE3 hex is 64 chars

    // Same file should produce same hash
    let transfer_id2 = manager.start_upload(drive_id, "/hashtest.txt", 4096).await;
    let hash2 = manager.process_transfer(&transfer_id2).await.unwrap();

    assert_eq!(hash, hash2);
}

#[tokio::test]
async fn test_different_files_different_hashes() {
    let manager = MockTransferManager::new();
    let drive_id = MockDriveId::new(1);

    let id1 = manager.start_upload(drive_id, "/file1.txt", 1024).await;
    let id2 = manager.start_upload(drive_id, "/file2.txt", 1024).await;
    let id3 = manager.start_upload(drive_id, "/file1.txt", 2048).await;

    let hash1 = manager.process_transfer(&id1).await.unwrap();
    let hash2 = manager.process_transfer(&id2).await.unwrap();
    let hash3 = manager.process_transfer(&id3).await.unwrap();

    // Different paths should have different hashes
    assert_ne!(hash1, hash2);
    // Same path but different size should have different hash
    assert_ne!(hash1, hash3);
}
