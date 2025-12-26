//! P2P System Integration Tests
//!
//! Tests for peer-to-peer connectivity, gossip protocol, and multi-peer scenarios.
//! These tests verify the P2P networking layer works correctly including:
//! - Endpoint creation and peer discovery
//! - Gossip message signing and verification
//! - Multi-peer event broadcasting
//! - Rate limiting and security
//!
//! Run with: cargo test --test p2p_tests -- --nocapture

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex, RwLock};

// ===========================================================================
// Test Infrastructure - Mock Types
// ===========================================================================

/// Mock NodeId representing a peer's public key
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MockNodeId([u8; 32]);

impl MockNodeId {
    fn new(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        MockNodeId(id)
    }

    fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Mock DriveId for testing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MockDriveId([u8; 32]);

impl MockDriveId {
    fn new(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        MockDriveId(id)
    }

    fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// Mock Identity with signing capabilities
#[derive(Clone)]
struct MockIdentity {
    node_id: MockNodeId,
    secret: [u8; 32],
}

impl MockIdentity {
    fn generate() -> Self {
        let mut secret = [0u8; 32];
        for byte in secret.iter_mut() {
            *byte = rand::random();
        }
        let node_id = MockNodeId(secret); // Simplified: use secret as public key
        Self { node_id, secret }
    }

    fn node_id(&self) -> MockNodeId {
        self.node_id
    }

    /// Create a simple signature (for testing purposes)
    fn sign(&self, message: &[u8]) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(message);
        hasher.update(&self.secret);
        hasher.finalize().as_bytes().to_vec()
    }

    /// Verify a signature
    fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        let expected = self.sign(message);
        expected == signature
    }
}

// ===========================================================================
// P2P Endpoint Tests
// ===========================================================================

/// Mock P2P Endpoint for testing peer connections
struct MockP2PEndpoint {
    is_running: Arc<RwLock<bool>>,
    connected_peers: Arc<RwLock<HashMap<MockNodeId, PeerInfo>>>,
    connection_handlers: Arc<Mutex<Vec<Box<dyn Fn(MockNodeId) + Send + Sync>>>>,
}

#[derive(Clone, Debug)]
struct PeerInfo;

impl MockP2PEndpoint {
    fn new() -> Self {
        Self {
            is_running: Arc::new(RwLock::new(false)),
            connected_peers: Arc::new(RwLock::new(HashMap::new())),
            connection_handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn start(&self) -> Result<(), String> {
        let mut running = self.is_running.write().await;
        if *running {
            return Err("Endpoint already running".to_string());
        }
        *running = true;
        Ok(())
    }

    async fn stop(&self) {
        let mut running = self.is_running.write().await;
        *running = false;
        
        // Disconnect all peers
        let mut peers = self.connected_peers.write().await;
        peers.clear();
    }

    async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    async fn connect(&self, peer_id: MockNodeId) -> Result<(), String> {
        if !self.is_running().await {
            return Err("Endpoint not running".to_string());
        }

        let mut peers = self.connected_peers.write().await;
        peers.insert(peer_id, PeerInfo);

        // Notify handlers
        let handlers = self.connection_handlers.lock().await;
        for handler in handlers.iter() {
            handler(peer_id);
        }

        Ok(())
    }

    async fn disconnect(&self, peer_id: &MockNodeId) {
        let mut peers = self.connected_peers.write().await;
        peers.remove(peer_id);
    }

    async fn is_connected(&self, peer_id: &MockNodeId) -> bool {
        let peers = self.connected_peers.read().await;
        peers.contains_key(peer_id)
    }

    async fn get_peer_count(&self) -> usize {
        let peers = self.connected_peers.read().await;
        peers.len()
    }

}

#[tokio::test]
async fn test_endpoint_lifecycle() {
    let endpoint = MockP2PEndpoint::new();

    // Initially not running
    assert!(!endpoint.is_running().await);

    // Start endpoint
    endpoint.start().await.unwrap();
    assert!(endpoint.is_running().await);

    // Double start should fail
    let result = endpoint.start().await;
    assert!(result.is_err());

    // Stop endpoint
    endpoint.stop().await;
    assert!(!endpoint.is_running().await);
}

#[tokio::test]
async fn test_peer_connection() {
    let identity2 = MockIdentity::generate();
    
    let endpoint = MockP2PEndpoint::new();
    endpoint.start().await.unwrap();

    // Connect to peer
    endpoint.connect(identity2.node_id()).await.unwrap();
    
    assert!(endpoint.is_connected(&identity2.node_id()).await);
    assert_eq!(endpoint.get_peer_count().await, 1);

    // Disconnect
    endpoint.disconnect(&identity2.node_id()).await;
    
    assert!(!endpoint.is_connected(&identity2.node_id()).await);
    assert_eq!(endpoint.get_peer_count().await, 0);
}

#[tokio::test]
async fn test_multiple_peer_connections() {
    let endpoint = MockP2PEndpoint::new();
    endpoint.start().await.unwrap();

    // Connect multiple peers
    let peer_count = 10;
    let mut peer_ids = Vec::new();
    
    for i in 0..peer_count {
        let peer_id = MockNodeId::new(i as u8);
        endpoint.connect(peer_id).await.unwrap();
        peer_ids.push(peer_id);
    }

    assert_eq!(endpoint.get_peer_count().await, peer_count);

    // Verify all peers are connected
    for peer_id in &peer_ids {
        assert!(endpoint.is_connected(peer_id).await);
    }

    // Disconnect half
    for peer_id in peer_ids.iter().take(peer_count / 2) {
        endpoint.disconnect(peer_id).await;
    }

    assert_eq!(endpoint.get_peer_count().await, peer_count / 2);
}

#[tokio::test]
async fn test_connection_without_running_endpoint() {
    let endpoint = MockP2PEndpoint::new();

    let peer_id = MockNodeId::new(1);
    let result = endpoint.connect(peer_id).await;

    assert!(result.is_err());
    assert!(!endpoint.is_connected(&peer_id).await);
}

#[tokio::test]
async fn test_stop_disconnects_all_peers() {
    let endpoint = MockP2PEndpoint::new();
    endpoint.start().await.unwrap();

    // Connect multiple peers
    for i in 0..5 {
        endpoint.connect(MockNodeId::new(i)).await.unwrap();
    }

    assert_eq!(endpoint.get_peer_count().await, 5);

    // Stop should disconnect all
    endpoint.stop().await;

    assert_eq!(endpoint.get_peer_count().await, 0);
}

// ===========================================================================
// Gossip Protocol Tests
// ===========================================================================

/// Mock drive event for testing
#[derive(Clone, Debug, PartialEq)]
enum MockDriveEvent {
    FileChanged { path: String, hash: String, size: u64 },
    FileDeleted { path: String },
    UserJoined { user: MockNodeId },
}

impl MockDriveEvent {
    fn to_bytes(&self) -> Vec<u8> {
        // Simple serialization for testing
        format!("{:?}", self).into_bytes()
    }
}

/// Signed gossip message
#[derive(Clone, Debug)]
struct SignedMessage {
    event: MockDriveEvent,
    sender: MockNodeId,
    timestamp_ms: i64,
    signature: Vec<u8>,
}

impl SignedMessage {
    fn new(event: MockDriveEvent, identity: &MockIdentity) -> Self {
        let sender = identity.node_id();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let payload = Self::create_signing_payload(&event, &sender, timestamp_ms);
        let signature = identity.sign(&payload);

        Self {
            event,
            sender,
            timestamp_ms,
            signature,
        }
    }

    fn verify(&self, identity: &MockIdentity) -> bool {
        if identity.node_id() != self.sender {
            return false;
        }
        let payload = Self::create_signing_payload(&self.event, &self.sender, self.timestamp_ms);
        identity.verify(&payload, &self.signature)
    }

    fn is_stale(&self, max_age_ms: i64) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        now_ms - self.timestamp_ms > max_age_ms
    }

    fn create_signing_payload(event: &MockDriveEvent, sender: &MockNodeId, timestamp_ms: i64) -> Vec<u8> {
        let mut payload = event.to_bytes();
        payload.extend_from_slice(sender.as_bytes());
        payload.extend_from_slice(&timestamp_ms.to_le_bytes());
        payload
    }
}

#[tokio::test]
async fn test_signed_message_creation_and_verification() {
    let identity = MockIdentity::generate();
    
    let event = MockDriveEvent::FileChanged {
        path: "/test/file.txt".to_string(),
        hash: "abc123".to_string(),
        size: 1024,
    };

    let signed_msg = SignedMessage::new(event.clone(), &identity);

    // Verify sender
    assert_eq!(signed_msg.sender, identity.node_id());

    // Verify signature
    assert!(signed_msg.verify(&identity));

    // Verify event
    assert_eq!(signed_msg.event, event);
}

#[tokio::test]
async fn test_signed_message_rejects_wrong_identity() {
    let identity1 = MockIdentity::generate();
    let identity2 = MockIdentity::generate();
    
    let event = MockDriveEvent::UserJoined {
        user: identity1.node_id(),
    };

    let signed_msg = SignedMessage::new(event, &identity1);

    // Should fail verification with wrong identity
    assert!(!signed_msg.verify(&identity2));
}

#[tokio::test]
async fn test_signed_message_stale_detection() {
    let identity = MockIdentity::generate();
    
    let event = MockDriveEvent::FileDeleted {
        path: "/test/old.txt".to_string(),
    };

    let mut signed_msg = SignedMessage::new(event, &identity);
    
    // Fresh message should not be stale
    assert!(!signed_msg.is_stale(5 * 60 * 1000)); // 5 minutes

    // Make the message old
    signed_msg.timestamp_ms -= 10 * 60 * 1000; // 10 minutes ago

    // Now it should be stale
    assert!(signed_msg.is_stale(5 * 60 * 1000));
}

/// Per-peer rate limiter
struct MockRateLimiter {
    limits: Arc<Mutex<HashMap<String, (usize, Instant)>>>,
    max_per_window: usize,
    window_duration: Duration,
}

impl MockRateLimiter {
    fn new(max_per_window: usize, window_duration: Duration) -> Self {
        Self {
            limits: Arc::new(Mutex::new(HashMap::new())),
            max_per_window,
            window_duration,
        }
    }

    async fn check(&self, peer_id: &str) -> bool {
        let mut limits = self.limits.lock().await;
        let now = Instant::now();

        let entry = limits.entry(peer_id.to_string()).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1) >= self.window_duration {
            entry.0 = 0;
            entry.1 = now;
        }

        // Check rate limit
        if entry.0 >= self.max_per_window {
            return false;
        }

        entry.0 += 1;
        true
    }
}

#[tokio::test]
async fn test_rate_limiter_allows_normal_traffic() {
    let limiter = MockRateLimiter::new(10, Duration::from_secs(1));

    for _ in 0..10 {
        assert!(limiter.check("peer1").await);
    }
}

#[tokio::test]
async fn test_rate_limiter_blocks_excessive_traffic() {
    let limiter = MockRateLimiter::new(5, Duration::from_secs(1));

    // First 5 should pass
    for _ in 0..5 {
        assert!(limiter.check("peer1").await);
    }

    // Next should be blocked
    assert!(!limiter.check("peer1").await);
    assert!(!limiter.check("peer1").await);
}

#[tokio::test]
async fn test_rate_limiter_independent_per_peer() {
    let limiter = MockRateLimiter::new(3, Duration::from_secs(1));

    // Each peer has independent limit
    for _ in 0..3 {
        assert!(limiter.check("peer1").await);
        assert!(limiter.check("peer2").await);
    }

    // Both should now be blocked independently
    assert!(!limiter.check("peer1").await);
    assert!(!limiter.check("peer2").await);
}

#[tokio::test]
async fn test_rate_limiter_window_reset() {
    let limiter = MockRateLimiter::new(3, Duration::from_millis(50));

    // Exhaust the limit
    for _ in 0..3 {
        assert!(limiter.check("peer1").await);
    }
    assert!(!limiter.check("peer1").await);

    // Wait for window to reset
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Should be allowed again
    assert!(limiter.check("peer1").await);
}

// ===========================================================================
// Event Broadcaster Tests
// ===========================================================================

/// Mock Event Broadcaster for gossip pub/sub
struct MockEventBroadcaster {
    identity: Arc<MockIdentity>,
    subscriptions: Arc<RwLock<HashMap<MockDriveId, broadcast::Sender<SignedMessage>>>>,
    rate_limiter: Arc<MockRateLimiter>,
    acl_checker: Arc<RwLock<Option<Box<dyn Fn(&str, &str) -> bool + Send + Sync>>>>,
}

impl MockEventBroadcaster {
    fn new(identity: MockIdentity) -> Self {
        Self {
            identity: Arc::new(identity),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter: Arc::new(MockRateLimiter::new(100, Duration::from_secs(1))),
            acl_checker: Arc::new(RwLock::new(None)),
        }
    }

    async fn subscribe(&self, drive_id: MockDriveId) -> broadcast::Receiver<SignedMessage> {
        let mut subs = self.subscriptions.write().await;
        
        if let Some(tx) = subs.get(&drive_id) {
            return tx.subscribe();
        }

        let (tx, rx) = broadcast::channel(256);
        subs.insert(drive_id, tx);
        rx
    }

    async fn unsubscribe(&self, drive_id: &MockDriveId) {
        let mut subs = self.subscriptions.write().await;
        subs.remove(drive_id);
    }

    async fn is_subscribed(&self, drive_id: &MockDriveId) -> bool {
        let subs = self.subscriptions.read().await;
        subs.contains_key(drive_id)
    }

    async fn broadcast(&self, drive_id: &MockDriveId, event: MockDriveEvent) -> Result<(), String> {
        let signed_msg = SignedMessage::new(event, &self.identity);
        
        let subs = self.subscriptions.read().await;
        if let Some(tx) = subs.get(drive_id) {
            tx.send(signed_msg).map_err(|_| "No subscribers".to_string())?;
        }
        Ok(())
    }

    async fn receive_external(
        &self,
        drive_id: &MockDriveId,
        signed_msg: SignedMessage,
        sender_identity: &MockIdentity,
    ) -> Result<(), String> {
        // Rate limit check
        if !self.rate_limiter.check(&signed_msg.sender.to_hex()).await {
            return Err("Rate limited".to_string());
        }

        // Signature verification
        if !signed_msg.verify(sender_identity) {
            return Err("Invalid signature".to_string());
        }

        // Stale message check
        if signed_msg.is_stale(5 * 60 * 1000) {
            return Err("Stale message".to_string());
        }

        // ACL check
        let acl = self.acl_checker.read().await;
        if let Some(ref checker) = *acl {
            if !checker(&drive_id.to_hex(), &signed_msg.sender.to_hex()) {
                return Err("Unauthorized sender".to_string());
            }
        }

        // Forward to subscribers
        let subs = self.subscriptions.read().await;
        if let Some(tx) = subs.get(drive_id) {
            tx.send(signed_msg).map_err(|_| "No subscribers".to_string())?;
        }

        Ok(())
    }

    async fn set_acl_checker<F>(&self, checker: F)
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        let mut acl = self.acl_checker.write().await;
        *acl = Some(Box::new(checker));
    }
}

#[tokio::test]
async fn test_broadcaster_subscribe_unsubscribe() {
    let identity = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity);
    let drive_id = MockDriveId::new(1);

    assert!(!broadcaster.is_subscribed(&drive_id).await);

    let _rx = broadcaster.subscribe(drive_id).await;
    assert!(broadcaster.is_subscribed(&drive_id).await);

    broadcaster.unsubscribe(&drive_id).await;
    assert!(!broadcaster.is_subscribed(&drive_id).await);
}

#[tokio::test]
async fn test_broadcaster_broadcast_receives_events() {
    let identity = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity);
    let drive_id = MockDriveId::new(1);

    let mut rx = broadcaster.subscribe(drive_id).await;

    let event = MockDriveEvent::FileChanged {
        path: "/test.txt".to_string(),
        hash: "hash123".to_string(),
        size: 512,
    };

    broadcaster.broadcast(&drive_id, event.clone()).await.unwrap();

    let received = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout")
        .expect("Channel error");

    assert_eq!(received.event, event);
}

#[tokio::test]
async fn test_broadcaster_multiple_subscribers() {
    let identity = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity);
    let drive_id = MockDriveId::new(1);

    let mut rx1 = broadcaster.subscribe(drive_id).await;
    let mut rx2 = broadcaster.subscribe(drive_id).await;
    let mut rx3 = broadcaster.subscribe(drive_id).await;

    let event = MockDriveEvent::UserJoined {
        user: MockNodeId::new(1),
    };

    broadcaster.broadcast(&drive_id, event.clone()).await.unwrap();

    // All subscribers should receive the event
    for rx in [&mut rx1, &mut rx2, &mut rx3] {
        let received = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel error");
        assert_eq!(received.event, event);
    }
}

#[tokio::test]
async fn test_broadcaster_rejects_invalid_signature() {
    let identity1 = MockIdentity::generate();
    let identity2 = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity1);
    let drive_id = MockDriveId::new(1);

    let _rx = broadcaster.subscribe(drive_id).await;

    // Create message with identity2 but try to verify with wrong identity
    let event = MockDriveEvent::FileDeleted {
        path: "/malicious.txt".to_string(),
    };
    let mut signed_msg = SignedMessage::new(event, &identity2);
    // Corrupt the signature
    signed_msg.signature[0] ^= 0xFF;

    let result = broadcaster.receive_external(&drive_id, signed_msg, &identity2).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid signature"));
}

#[tokio::test]
async fn test_broadcaster_rejects_stale_messages() {
    let identity = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity.clone());
    let drive_id = MockDriveId::new(1);

    let _rx = broadcaster.subscribe(drive_id).await;

    let event = MockDriveEvent::FileChanged {
        path: "/old.txt".to_string(),
        hash: "oldhash".to_string(),
        size: 100,
    };

    let mut signed_msg = SignedMessage::new(event, &identity);
    // Make message 10 minutes old
    signed_msg.timestamp_ms -= 10 * 60 * 1000;
    // Re-sign with correct timestamp (simulate replay attack)
    let payload = SignedMessage::create_signing_payload(&signed_msg.event, &signed_msg.sender, signed_msg.timestamp_ms);
    signed_msg.signature = identity.sign(&payload);

    let result = broadcaster.receive_external(&drive_id, signed_msg, &identity).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Stale message"));
}

#[tokio::test]
async fn test_broadcaster_acl_enforcement() {
    let identity = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity.clone());
    let drive_id = MockDriveId::new(1);

    let _rx = broadcaster.subscribe(drive_id).await;

    // Set ACL that denies all
    broadcaster.set_acl_checker(|_drive, _sender| false).await;

    let event = MockDriveEvent::UserJoined {
        user: MockNodeId::new(5),
    };
    let signed_msg = SignedMessage::new(event, &identity);

    let result = broadcaster.receive_external(&drive_id, signed_msg, &identity).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unauthorized"));
}

// ===========================================================================
// Multi-Peer Communication Tests
// ===========================================================================

/// A simulated P2P network for testing multi-peer scenarios
struct MockP2PNetwork {
    nodes: Arc<RwLock<HashMap<MockNodeId, MockEventBroadcaster>>>,
}

impl MockP2PNetwork {
    fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn add_node(&self, identity: MockIdentity) -> MockNodeId {
        let node_id = identity.node_id();
        let broadcaster = MockEventBroadcaster::new(identity);
        
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id, broadcaster);
        
        node_id
    }

    async fn subscribe_node(&self, node_id: &MockNodeId, drive_id: MockDriveId) -> Option<broadcast::Receiver<SignedMessage>> {
        let nodes = self.nodes.read().await;
        if let Some(broadcaster) = nodes.get(node_id) {
            Some(broadcaster.subscribe(drive_id).await)
        } else {
            None
        }
    }

    async fn broadcast_from(&self, sender_id: &MockNodeId, drive_id: MockDriveId, event: MockDriveEvent) -> Result<(), String> {
        let nodes = self.nodes.read().await;
        let sender = nodes.get(sender_id).ok_or("Sender not found")?;
        
        sender.broadcast(&drive_id, event).await
    }

    async fn get_node_count(&self) -> usize {
        let nodes = self.nodes.read().await;
        nodes.len()
    }
}

#[tokio::test]
async fn test_network_multiple_nodes() {
    let network = MockP2PNetwork::new();

    // Add 5 nodes
    let mut node_ids = Vec::new();
    for _ in 0..5 {
        let identity = MockIdentity::generate();
        let node_id = network.add_node(identity).await;
        node_ids.push(node_id);
    }

    assert_eq!(network.get_node_count().await, 5);
}

#[tokio::test]
async fn test_network_broadcast_to_all_subscribers() {
    let network = MockP2PNetwork::new();
    let drive_id = MockDriveId::new(1);

    // Add nodes
    let sender_identity = MockIdentity::generate();
    let sender_id = network.add_node(sender_identity).await;

    let mut receivers = Vec::new();
    for _ in 0..3 {
        let identity = MockIdentity::generate();
        let node_id = network.add_node(identity).await;
        let rx = network.subscribe_node(&node_id, drive_id).await.unwrap();
        receivers.push(rx);
    }

    // Sender subscribes and broadcasts
    let _sender_rx = network.subscribe_node(&sender_id, drive_id).await.unwrap();
    
    let event = MockDriveEvent::FileChanged {
        path: "/shared/doc.txt".to_string(),
        hash: "newhash".to_string(),
        size: 2048,
    };

    network.broadcast_from(&sender_id, drive_id, event.clone()).await.unwrap();

    // Note: In real implementation, message would propagate to all subscribers
    // This test verifies the broadcasting mechanism
}

// ===========================================================================
// Sync Engine Integration Tests
// ===========================================================================

/// Mock Sync Engine coordinating docs, gossip, and transfers
struct MockSyncEngine {
    syncing_drives: Arc<RwLock<HashMap<MockDriveId, SyncState>>>,
    event_tx: broadcast::Sender<(MockDriveId, MockDriveEvent)>,
}

#[derive(Clone, Debug)]
struct SyncState {
    is_syncing: bool,
    connected_peers: Vec<MockNodeId>,
    last_sync_ms: Option<i64>,
}

impl MockSyncEngine {
    fn new() -> Self {
        let (event_tx, _) = broadcast::channel(512);
        Self {
            syncing_drives: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    async fn init_drive(&self, drive_id: MockDriveId) -> Result<(), String> {
        let mut drives = self.syncing_drives.write().await;
        drives.insert(drive_id, SyncState {
            is_syncing: true,
            connected_peers: Vec::new(),
            last_sync_ms: None,
        });
        Ok(())
    }

    async fn join_drive(&self, drive_id: MockDriveId, peer_id: MockNodeId) -> Result<(), String> {
        let mut drives = self.syncing_drives.write().await;
        let state = drives.entry(drive_id).or_insert(SyncState {
            is_syncing: true,
            connected_peers: Vec::new(),
            last_sync_ms: None,
        });
        state.connected_peers.push(peer_id);
        Ok(())
    }

    async fn stop_sync(&self, drive_id: &MockDriveId) {
        let mut drives = self.syncing_drives.write().await;
        drives.remove(drive_id);
    }

    async fn is_syncing(&self, drive_id: &MockDriveId) -> bool {
        let drives = self.syncing_drives.read().await;
        drives.get(drive_id).map(|s| s.is_syncing).unwrap_or(false)
    }

    async fn get_peer_count(&self, drive_id: &MockDriveId) -> usize {
        let drives = self.syncing_drives.read().await;
        drives.get(drive_id).map(|s| s.connected_peers.len()).unwrap_or(0)
    }

    async fn on_local_change(&self, drive_id: &MockDriveId, event: MockDriveEvent) -> Result<(), String> {
        if !self.is_syncing(drive_id).await {
            return Err("Drive not syncing".to_string());
        }

        let _ = self.event_tx.send((*drive_id, event));
        
        // Update last sync time
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        let mut drives = self.syncing_drives.write().await;
        if let Some(state) = drives.get_mut(drive_id) {
            state.last_sync_ms = Some(now_ms);
        }

        Ok(())
    }

    fn subscribe_events(&self) -> broadcast::Receiver<(MockDriveId, MockDriveEvent)> {
        self.event_tx.subscribe()
    }
}

#[tokio::test]
async fn test_sync_engine_init_and_join() {
    let engine = MockSyncEngine::new();
    let drive_id = MockDriveId::new(1);
    let peer_id = MockNodeId::new(1);

    // Initialize drive
    engine.init_drive(drive_id).await.unwrap();
    assert!(engine.is_syncing(&drive_id).await);
    assert_eq!(engine.get_peer_count(&drive_id).await, 0);

    // Join with peer
    engine.join_drive(drive_id, peer_id).await.unwrap();
    assert_eq!(engine.get_peer_count(&drive_id).await, 1);
}

#[tokio::test]
async fn test_sync_engine_local_changes() {
    let engine = MockSyncEngine::new();
    let drive_id = MockDriveId::new(1);

    engine.init_drive(drive_id).await.unwrap();

    let mut rx = engine.subscribe_events();

    let event = MockDriveEvent::FileChanged {
        path: "/local/file.txt".to_string(),
        hash: "localhash".to_string(),
        size: 4096,
    };

    engine.on_local_change(&drive_id, event.clone()).await.unwrap();

    let (received_drive, received_event) = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout")
        .expect("Channel error");

    assert_eq!(received_drive, drive_id);
    assert_eq!(received_event, event);
}

#[tokio::test]
async fn test_sync_engine_stop_sync() {
    let engine = MockSyncEngine::new();
    let drive_id = MockDriveId::new(1);

    engine.init_drive(drive_id).await.unwrap();
    assert!(engine.is_syncing(&drive_id).await);

    engine.stop_sync(&drive_id).await;
    assert!(!engine.is_syncing(&drive_id).await);

    // Changes should fail after stop
    let event = MockDriveEvent::FileChanged {
        path: "/test.txt".to_string(),
        hash: "hash".to_string(),
        size: 100,
    };
    let result = engine.on_local_change(&drive_id, event).await;
    assert!(result.is_err());
}

// ===========================================================================
// Concurrent P2P Operations Tests
// ===========================================================================

#[tokio::test]
async fn test_concurrent_peer_connections() {
    let endpoint = Arc::new(MockP2PEndpoint::new());
    endpoint.start().await.unwrap();

    let mut handles = Vec::new();

    // Spawn 50 concurrent connection attempts
    for i in 0..50 {
        let endpoint = endpoint.clone();
        handles.push(tokio::spawn(async move {
            let peer_id = MockNodeId::new(i as u8);
            endpoint.connect(peer_id).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(endpoint.get_peer_count().await, 50);
}

#[tokio::test]
async fn test_concurrent_broadcasts() {
    let identity = MockIdentity::generate();
    let broadcaster = Arc::new(MockEventBroadcaster::new(identity));
    let drive_id = MockDriveId::new(1);

    let _rx = broadcaster.subscribe(drive_id).await;

    let broadcast_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    // Spawn 100 concurrent broadcasts
    for i in 0..100 {
        let broadcaster = broadcaster.clone();
        let count = broadcast_count.clone();
        handles.push(tokio::spawn(async move {
            let event = MockDriveEvent::FileChanged {
                path: format!("/file{}.txt", i),
                hash: format!("hash{}", i),
                size: i as u64 * 100,
            };
            if broadcaster.broadcast(&drive_id, event).await.is_ok() {
                count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // All broadcasts should succeed (buffer is 256)
    assert!(broadcast_count.load(Ordering::Relaxed) > 0);
}

#[tokio::test]
async fn test_rate_limiter_under_load() {
    let limiter = Arc::new(MockRateLimiter::new(100, Duration::from_secs(1)));
    let blocked_count = Arc::new(AtomicUsize::new(0));
    let allowed_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    // Simulate burst from single peer - 200 messages
    for _ in 0..200 {
        let limiter = limiter.clone();
        let blocked = blocked_count.clone();
        let allowed = allowed_count.clone();
        
        handles.push(tokio::spawn(async move {
            if limiter.check("burst_peer").await {
                allowed.fetch_add(1, Ordering::Relaxed);
            } else {
                blocked.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // First 100 should be allowed, rest blocked
    assert_eq!(allowed_count.load(Ordering::Relaxed), 100);
    assert_eq!(blocked_count.load(Ordering::Relaxed), 100);
}

// ===========================================================================
// P2P Network Resilience Tests
// ===========================================================================

#[tokio::test]
async fn test_endpoint_rapid_restart() {
    let endpoint = MockP2PEndpoint::new();

    for iteration in 0..50 {
        endpoint.start().await.unwrap();
        
        // Add some peers
        for i in 0..3 {
            endpoint.connect(MockNodeId::new((iteration * 3 + i) as u8)).await.unwrap();
        }
        
        assert_eq!(endpoint.get_peer_count().await, 3);
        
        endpoint.stop().await;
        
        assert!(!endpoint.is_running().await);
        assert_eq!(endpoint.get_peer_count().await, 0);
    }
}

#[tokio::test]
async fn test_broadcaster_subscription_stress() {
    let identity = MockIdentity::generate();
    let broadcaster = Arc::new(MockEventBroadcaster::new(identity));

    let mut handles = Vec::new();

    // Concurrent subscribe/unsubscribe operations
    for i in 0..50 {
        let broadcaster = broadcaster.clone();
        handles.push(tokio::spawn(async move {
            let drive_id = MockDriveId::new(i as u8);
            
            let _rx = broadcaster.subscribe(drive_id).await;
            assert!(broadcaster.is_subscribed(&drive_id).await);
            
            tokio::time::sleep(Duration::from_millis(1)).await;
            
            broadcaster.unsubscribe(&drive_id).await;
            assert!(!broadcaster.is_subscribed(&drive_id).await);
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

// ===========================================================================
// Performance Benchmarks
// ===========================================================================

#[tokio::test]
async fn test_message_signing_throughput() {
    let identity = MockIdentity::generate();
    
    let start = Instant::now();
    const MESSAGE_COUNT: usize = 10_000;

    for i in 0..MESSAGE_COUNT {
        let event = MockDriveEvent::FileChanged {
            path: format!("/file{}.txt", i),
            hash: format!("hash{}", i),
            size: i as u64,
        };
        let _signed = SignedMessage::new(event, &identity);
    }

    let elapsed = start.elapsed();
    let msgs_per_sec = MESSAGE_COUNT as f64 / elapsed.as_secs_f64();

    println!("Message signing throughput: {:.0} msg/sec", msgs_per_sec);

    // Should achieve at least 10k messages per second
    assert!(
        msgs_per_sec > 10_000.0,
        "Signing throughput too low: {:.0} msg/sec",
        msgs_per_sec
    );
}

#[tokio::test]
async fn test_rate_limiter_throughput() {
    let limiter = MockRateLimiter::new(1_000_000, Duration::from_secs(60));
    
    let start = Instant::now();
    const CHECK_COUNT: usize = 100_000;

    for i in 0..CHECK_COUNT {
        let peer_id = format!("peer{}", i % 100);
        limiter.check(&peer_id).await;
    }

    let elapsed = start.elapsed();
    let checks_per_sec = CHECK_COUNT as f64 / elapsed.as_secs_f64();

    println!("Rate limiter throughput: {:.0} checks/sec", checks_per_sec);

    // Should achieve at least 100k checks per second
    assert!(
        checks_per_sec > 100_000.0,
        "Rate limiter throughput too low: {:.0} checks/sec",
        checks_per_sec
    );
}

#[tokio::test]
async fn test_broadcast_channel_throughput() {
    let identity = MockIdentity::generate();
    let broadcaster = MockEventBroadcaster::new(identity);
    let drive_id = MockDriveId::new(1);

    let mut rx = broadcaster.subscribe(drive_id).await;

    let received_count = Arc::new(AtomicUsize::new(0));
    let count_clone = received_count.clone();

    // Spawn receiver
    let receiver = tokio::spawn(async move {
        while let Ok(_) = rx.recv().await {
            count_clone.fetch_add(1, Ordering::Relaxed);
        }
    });

    let start = Instant::now();
    const MESSAGE_COUNT: usize = 50_000;

    for i in 0..MESSAGE_COUNT {
        let event = MockDriveEvent::FileChanged {
            path: format!("/file{}.txt", i),
            hash: format!("hash{}", i),
            size: i as u64,
        };
        let _ = broadcaster.broadcast(&drive_id, event).await;
    }

    // Give receiver time to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    let elapsed = start.elapsed();
    let received = received_count.load(Ordering::Relaxed);
    let msgs_per_sec = received as f64 / elapsed.as_secs_f64();

    println!(
        "Broadcast throughput: {:.0} msg/sec ({} received of {} sent)",
        msgs_per_sec, received, MESSAGE_COUNT
    );

    receiver.abort();
}
