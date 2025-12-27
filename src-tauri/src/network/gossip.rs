//! Event broadcasting via iroh-gossip protocol
//!
//! Provides pub/sub for real-time drive events between peers.
//! Each drive has its own gossip topic derived from its DriveId.
//!
//! All messages are cryptographically signed for authentication.
//! Sender authorization is verified against ACLs when a security store is configured.
//! Per-peer rate limiting prevents DoS attacks via message flooding.

#![allow(dead_code)]

use crate::core::{
    send_with_backpressure, DriveEvent, DriveEventDto, DriveId, SignedGossipMessage,
};
use crate::crypto::Identity;
use anyhow::Result;
use iroh::protocol::ProtocolHandler;
use iroh::Endpoint;
use iroh_gossip::net::Gossip;
use iroh_gossip::proto::TopicId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;

/// Maximum age of a gossip message before it's considered stale (5 minutes)
const MAX_MESSAGE_AGE_MS: i64 = 5 * 60 * 1000;

/// Maximum messages per peer per second before rate limiting kicks in
const PEER_RATE_LIMIT_PER_SEC: usize = 100;

/// Rate limit window duration in seconds
const RATE_LIMIT_WINDOW_SECS: u64 = 1;

/// Per-peer rate limiter to prevent DoS attacks
#[derive(Clone)]
struct PeerRateLimiter {
    /// Message counts per peer (peer_id -> (count, window_start))
    limits: Arc<Mutex<HashMap<String, (usize, Instant)>>>,
    /// Maximum messages per window
    max_per_window: usize,
    /// Window duration
    window_secs: u64,
}

impl PeerRateLimiter {
    fn new(max_per_window: usize, window_secs: u64) -> Self {
        Self {
            limits: Arc::new(Mutex::new(HashMap::new())),
            max_per_window,
            window_secs,
        }
    }

    /// Check if a peer should be rate limited
    /// Returns true if the message should be processed, false if rate limited
    async fn check(&self, peer_id: &str) -> bool {
        let mut limits = self.limits.lock().await;
        let now = Instant::now();

        let entry = limits.entry(peer_id.to_string()).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1).as_secs() >= self.window_secs {
            entry.0 = 0;
            entry.1 = now;
        }

        // Check if rate limited
        if entry.0 >= self.max_per_window {
            return false;
        }

        // Increment count
        entry.0 += 1;
        true
    }

    /// Cleanup stale entries (peers we haven't heard from in a while)
    async fn cleanup(&self) {
        let mut limits = self.limits.lock().await;
        let now = Instant::now();
        // Remove entries older than 60 seconds
        limits.retain(|_, (_, start)| now.duration_since(*start).as_secs() < 60);
    }
}

/// Type alias for the ACL checking callback
/// Takes (drive_id, sender_node_id) and returns true if sender is authorized
pub type AclChecker = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

/// Manages gossip subscriptions per drive for real-time event broadcasting
pub struct EventBroadcaster {
    /// The gossip protocol instance (wrapped in RwLock<Option<>> for safe shutdown)
    gossip: RwLock<Option<Arc<Gossip>>>,
    /// Active topic subscriptions per drive
    subscriptions: RwLock<HashMap<DriveId, TopicSubscription>>,
    /// Channel to forward events to Tauri frontend
    frontend_tx: broadcast::Sender<DriveEventDto>,
    /// Flag to indicate if shutdown has been called
    shutdown_flag: AtomicBool,
    /// Our identity for signing outbound messages
    identity: Arc<Identity>,
    /// Optional ACL checker for sender authorization
    acl_checker: RwLock<Option<AclChecker>>,
}

/// Holds state for a single drive's gossip subscription
struct TopicSubscription {
    /// The gossip topic ID for this drive
    _topic_id: TopicId,
    /// Handle to the receiver task
    receiver_task: JoinHandle<()>,
}

impl EventBroadcaster {
    /// Create a new EventBroadcaster from an Iroh endpoint
    pub async fn new(endpoint: &Endpoint, identity: Arc<Identity>) -> Result<Self> {
        let gossip = Gossip::builder().spawn(endpoint.clone()).await?;

        // Create broadcast channel for frontend events (buffer 256 events)
        let (frontend_tx, _) = broadcast::channel(256);

        tracing::info!("EventBroadcaster initialized with message signing enabled");

        Ok(Self {
            gossip: RwLock::new(Some(Arc::new(gossip))),
            subscriptions: RwLock::new(HashMap::new()),
            frontend_tx,
            shutdown_flag: AtomicBool::new(false),
            identity,
            acl_checker: RwLock::new(None),
        })
    }

    /// Set the ACL checker for sender authorization
    ///
    /// This should be called after the SecurityStore is initialized.
    /// When set, incoming gossip messages will only be processed if the
    /// sender has at least Read permission on the drive.
    pub async fn set_acl_checker(&self, checker: AclChecker) {
        let mut guard = self.acl_checker.write().await;
        *guard = Some(checker);
        tracing::info!("ACL checker configured for gossip sender authorization");
    }

    /// Get the underlying gossip instance (if initialized)
    pub async fn gossip(&self) -> Option<Arc<Gossip>> {
        self.get_gossip().await
    }

    /// Get a reference to the gossip instance for operations
    /// Returns None if shutdown has been called
    async fn get_gossip(&self) -> Option<Arc<Gossip>> {
        let guard = self.gossip.read().await;
        guard.clone()
    }

    /// Subscribe to a drive's gossip topic
    ///
    /// This starts receiving events from other peers for the given drive.
    /// Events are automatically forwarded to the frontend channel.
    pub async fn subscribe(&self, drive_id: DriveId) -> Result<()> {
        self.subscribe_with_peers(drive_id, vec![]).await
    }

    /// Subscribe to a drive's gossip topic with optional bootstrap peers
    ///
    /// This starts receiving events from other peers for the given drive.
    /// Bootstrap peers help with initial connectivity for joined drives.
    pub async fn subscribe_with_peers(
        &self,
        drive_id: DriveId,
        bootstrap_peers: Vec<iroh::NodeId>,
    ) -> Result<()> {
        let topic_id = self.drive_to_topic(&drive_id);

        // Check if already subscribed
        {
            let subs = self.subscriptions.read().await;
            if subs.contains_key(&drive_id) {
                tracing::debug!("Already subscribed to drive {}", drive_id);
                return Ok(());
            }
        }

        // Get gossip instance
        let gossip = self
            .get_gossip()
            .await
            .ok_or_else(|| anyhow::anyhow!("EventBroadcaster has been shut down"))?;

        // Subscribe to the topic with bootstrap peers
        // For joined drives, these should be peers from the doc ticket
        if !bootstrap_peers.is_empty() {
            tracing::info!(
                "Subscribing to gossip topic for drive {} with {} bootstrap peers",
                drive_id,
                bootstrap_peers.len()
            );
        }
        let topic = gossip.subscribe(topic_id, bootstrap_peers)?;
        let (_sender, mut receiver) = topic.split();

        // Clone ACL checker for the spawned task
        let acl_checker = self.acl_checker.read().await.clone();

        // Create per-peer rate limiter for this topic
        let rate_limiter = PeerRateLimiter::new(PEER_RATE_LIMIT_PER_SEC, RATE_LIMIT_WINDOW_SECS);

        // Spawn receiver task to forward events to frontend
        let frontend_tx = self.frontend_tx.clone();
        let drive_id_hex = drive_id.to_hex();
        let drive_id_for_task = drive_id;

        let receiver_task = tokio::spawn(async move {
            use futures_lite::StreamExt;

            tracing::debug!("Started gossip receiver for drive {}", drive_id_hex);

            // Periodically cleanup rate limiter entries
            let rate_limiter_for_cleanup = rate_limiter.clone();
            let cleanup_task = tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    rate_limiter_for_cleanup.cleanup().await;
                }
            });

            while let Some(event_result) = receiver.next().await {
                match event_result {
                    Ok(event) => {
                        use iroh_gossip::net::{Event, GossipEvent};

                        match event {
                            Event::Gossip(GossipEvent::Received(msg)) => {
                                // Deserialize the signed message envelope
                                match serde_json::from_slice::<SignedGossipMessage>(&msg.content) {
                                    Ok(signed_msg) => {
                                        // SECURITY: Rate limit check BEFORE signature verification
                                        // This prevents DoS via CPU-intensive signature verification
                                        let sender_id = signed_msg.sender.to_hex();
                                        if !rate_limiter.check(&sender_id).await {
                                            tracing::warn!(
                                                "Rate limited gossip messages from peer {} for drive {}",
                                                signed_msg.sender.short_string(),
                                                drive_id_hex
                                            );
                                            continue;
                                        }

                                        // Verify the signature
                                        if let Err(e) = signed_msg.verify() {
                                            tracing::warn!(
                                                "Rejected gossip message with invalid signature: {} from {:?}",
                                                e,
                                                msg.delivered_from
                                            );
                                            continue;
                                        }

                                        // Check for replay attack (stale messages)
                                        if signed_msg.is_stale(MAX_MESSAGE_AGE_MS) {
                                            tracing::warn!(
                                                "Rejected stale gossip message from {} (age: {}ms)",
                                                signed_msg.sender.short_string(),
                                                chrono::Utc::now().timestamp_millis()
                                                    - signed_msg.timestamp_ms
                                            );
                                            continue;
                                        }

                                        // SECURITY: Check if sender is authorized for this drive
                                        if let Some(ref checker) = acl_checker {
                                            let sender_hex = signed_msg.sender.to_hex();
                                            if !checker(&drive_id_hex, &sender_hex) {
                                                tracing::warn!(
                                                    "Rejected gossip message from unauthorized sender {} for drive {}",
                                                    signed_msg.sender.short_string(),
                                                    drive_id_hex
                                                );
                                                continue;
                                            }
                                        }

                                        // Message is authenticated and authorized - extract the event
                                        let drive_event = signed_msg.event;
                                        let dto = DriveEventDto::from_event(
                                            &drive_id_for_task.to_hex(),
                                            &drive_event,
                                        );

                                        tracing::debug!(
                                            "Received authenticated gossip event: {} for drive {} from {}",
                                            dto.event_type,
                                            drive_id_hex,
                                            signed_msg.sender.short_string()
                                        );

                                        // Forward to frontend with backpressure monitoring
                                        send_with_backpressure(
                                            &frontend_tx,
                                            dto,
                                            "gossip_frontend",
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to deserialize gossip message: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            Event::Gossip(GossipEvent::Joined(peers)) => {
                                tracing::info!(
                                    "Joined gossip topic for drive {} with {} peers",
                                    drive_id_hex,
                                    peers.len()
                                );
                            }
                            Event::Gossip(GossipEvent::NeighborUp(peer)) => {
                                tracing::debug!("Peer {} joined drive {}", peer, drive_id_hex);
                            }
                            Event::Gossip(GossipEvent::NeighborDown(peer)) => {
                                tracing::debug!("Peer {} left drive {}", peer, drive_id_hex);
                            }
                            Event::Lagged => {
                                tracing::warn!("Gossip receiver lagged for drive {}", drive_id_hex);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Gossip receiver error: {}", e);
                    }
                }
            }

            // Abort the cleanup task when receiver ends
            cleanup_task.abort();
            tracing::debug!("Gossip receiver ended for drive {}", drive_id_hex);
        });

        // Store the subscription
        let mut subs = self.subscriptions.write().await;
        subs.insert(
            drive_id,
            TopicSubscription {
                _topic_id: topic_id,
                receiver_task,
            },
        );

        tracing::info!("Subscribed to gossip topic for drive {}", drive_id);
        Ok(())
    }

    /// Unsubscribe from a drive's gossip topic
    pub async fn unsubscribe(&self, drive_id: &DriveId) {
        let mut subs = self.subscriptions.write().await;
        if let Some(sub) = subs.remove(drive_id) {
            sub.receiver_task.abort();
            tracing::info!("Unsubscribed from gossip topic for drive {}", drive_id);
        }
    }

    /// Broadcast an event to all peers subscribed to a drive
    ///
    /// Messages are automatically signed with our identity for authentication.
    pub async fn broadcast(&self, drive_id: &DriveId, event: DriveEvent) -> Result<()> {
        let topic_id = self.drive_to_topic(drive_id);

        // Create signed message envelope
        let signed_msg = SignedGossipMessage::new(event.clone(), &self.identity);

        // Serialize the signed message
        let data = serde_json::to_vec(&signed_msg)?;

        // Get gossip instance
        let gossip = self
            .get_gossip()
            .await
            .ok_or_else(|| anyhow::anyhow!("EventBroadcaster has been shut down"))?;

        // Get a sender for this topic
        let topic = gossip.subscribe(topic_id, vec![])?;
        let (sender, _receiver) = topic.split();

        // Broadcast the signed message
        sender.broadcast(data.into()).await?;

        tracing::debug!(
            "Broadcast signed {} event for drive {}",
            event.event_type(),
            drive_id
        );

        Ok(())
    }

    /// Get a receiver for frontend events
    ///
    /// Returns a broadcast receiver that gets all events from all subscribed drives.
    /// Used by the Tauri event forwarding task.
    pub fn subscribe_frontend(&self) -> broadcast::Receiver<DriveEventDto> {
        self.frontend_tx.subscribe()
    }

    /// Check if subscribed to a drive
    pub async fn is_subscribed(&self, drive_id: &DriveId) -> bool {
        let subs = self.subscriptions.read().await;
        subs.contains_key(drive_id)
    }

    /// Get list of subscribed drive IDs
    pub async fn subscribed_drives(&self) -> Vec<DriveId> {
        let subs = self.subscriptions.read().await;
        subs.keys().cloned().collect()
    }

    /// Convert DriveId to TopicId (deterministic mapping)
    fn drive_to_topic(&self, drive_id: &DriveId) -> TopicId {
        TopicId::from_bytes(*drive_id.as_bytes())
    }

    /// Gracefully shutdown the EventBroadcaster
    ///
    /// This must be called before the Tokio runtime is destroyed to avoid
    /// panics from async Drop implementations in iroh-gossip.
    pub async fn shutdown(&self) {
        if self.shutdown_flag.swap(true, Ordering::SeqCst) {
            // Already shutting down or shut down
            return;
        }

        tracing::info!("EventBroadcaster shutting down...");

        // Abort all receiver tasks first
        {
            let mut subs = self.subscriptions.write().await;
            for (drive_id, sub) in subs.drain() {
                sub.receiver_task.abort();
                tracing::debug!("Aborted receiver task for drive {}", drive_id);
            }
        }

        // Shutdown the gossip protocol
        // Note: Gossip::shutdown() is called through the inner Arc when it's dropped
        // but we can explicitly quit to trigger a clean shutdown before runtime destruction
        {
            let mut gossip = self.gossip.write().await;
            if let Some(g) = gossip.take() {
                g.shutdown().await;
            }
        }

        tracing::info!("EventBroadcaster shutdown complete");
    }

    /// Check if shutdown has been initiated
    pub fn is_shutdown(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }
}

impl Drop for EventBroadcaster {
    fn drop(&mut self) {
        // Only log if we haven't been gracefully shutdown
        if !self.shutdown_flag.load(Ordering::SeqCst) {
            tracing::warn!("EventBroadcaster dropped without graceful shutdown - this may cause panics if Tokio runtime is already gone");
        }
        tracing::debug!("EventBroadcaster dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DriveEvent;
    use crate::crypto::Identity;
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn test_drive_to_topic_deterministic() {
        // Create two identical DriveIds and verify they map to the same TopicId
        let bytes = [1u8; 32];
        let drive_id1 = DriveId(bytes);
        let drive_id2 = DriveId(bytes);

        let topic1 = TopicId::from_bytes(*drive_id1.as_bytes());
        let topic2 = TopicId::from_bytes(*drive_id2.as_bytes());

        assert_eq!(topic1.as_bytes(), topic2.as_bytes());
    }

    #[test]
    fn test_drive_to_topic_unique_for_different_drives() {
        let bytes1 = [1u8; 32];
        let bytes2 = [2u8; 32];
        let drive_id1 = DriveId(bytes1);
        let drive_id2 = DriveId(bytes2);

        let topic1 = TopicId::from_bytes(*drive_id1.as_bytes());
        let topic2 = TopicId::from_bytes(*drive_id2.as_bytes());

        assert_ne!(topic1.as_bytes(), topic2.as_bytes());
    }

    #[test]
    fn test_peer_rate_limiter_creation() {
        let limiter = PeerRateLimiter::new(100, 1);
        assert_eq!(limiter.max_per_window, 100);
        assert_eq!(limiter.window_secs, 1);
    }

    #[tokio::test]
    async fn test_peer_rate_limiter_allows_within_limit() {
        let limiter = PeerRateLimiter::new(5, 1);

        for _ in 0..5 {
            assert!(limiter.check("peer1").await);
        }
    }

    #[tokio::test]
    async fn test_peer_rate_limiter_blocks_over_limit() {
        let limiter = PeerRateLimiter::new(3, 1);

        // First 3 should pass
        assert!(limiter.check("peer1").await);
        assert!(limiter.check("peer1").await);
        assert!(limiter.check("peer1").await);

        // 4th should be blocked
        assert!(!limiter.check("peer1").await);
        assert!(!limiter.check("peer1").await);
    }

    #[tokio::test]
    async fn test_peer_rate_limiter_independent_per_peer() {
        let limiter = PeerRateLimiter::new(2, 1);

        // Each peer gets their own limit
        assert!(limiter.check("peer1").await);
        assert!(limiter.check("peer1").await);
        assert!(!limiter.check("peer1").await);

        // peer2 has a fresh limit
        assert!(limiter.check("peer2").await);
        assert!(limiter.check("peer2").await);
        assert!(!limiter.check("peer2").await);
    }

    #[tokio::test]
    async fn test_peer_rate_limiter_window_reset() {
        let limiter = PeerRateLimiter::new(2, 0); // 0 second window = immediate reset

        assert!(limiter.check("peer1").await);
        assert!(limiter.check("peer1").await);

        // Window should reset since window_secs is 0
        assert!(limiter.check("peer1").await);
    }

    #[tokio::test]
    async fn test_peer_rate_limiter_cleanup() {
        let limiter = PeerRateLimiter::new(10, 1);

        // Add some entries
        limiter.check("peer1").await;
        limiter.check("peer2").await;
        limiter.check("peer3").await;

        // Check entries exist
        {
            let limits = limiter.limits.lock().await;
            assert_eq!(limits.len(), 3);
        }

        // Cleanup should not remove recent entries
        limiter.cleanup().await;

        {
            let limits = limiter.limits.lock().await;
            assert_eq!(limits.len(), 3);
        }
    }

    #[test]
    fn test_max_message_age_constant() {
        assert_eq!(MAX_MESSAGE_AGE_MS, 5 * 60 * 1000);
    }

    #[test]
    fn test_rate_limit_constants() {
        assert_eq!(PEER_RATE_LIMIT_PER_SEC, 100);
        assert_eq!(RATE_LIMIT_WINDOW_SECS, 1);
    }

    #[test]
    fn test_signed_gossip_message_creation() {
        let identity = Identity::generate();
        let node_id = identity.node_id();

        let event = DriveEvent::UserJoined {
            user: node_id,
            timestamp: Utc::now(),
        };

        let signed_msg = SignedGossipMessage::new(event.clone(), &identity);

        assert_eq!(signed_msg.sender, node_id);
        assert!(!signed_msg.signature.is_empty());
        assert!(signed_msg.timestamp_ms > 0);
    }

    #[test]
    fn test_signed_gossip_message_verification() {
        let identity = Identity::generate();

        let event = DriveEvent::FileChanged {
            path: PathBuf::from("/test/file.txt"),
            hash: "abc123".to_string(),
            size: 1024,
            modified_by: identity.node_id(),
            timestamp: Utc::now(),
        };

        let signed_msg = SignedGossipMessage::new(event, &identity);

        // Should verify successfully
        assert!(signed_msg.verify().is_ok());
    }

    #[test]
    fn test_signed_gossip_message_tamper_detection() {
        let identity = Identity::generate();

        let event = DriveEvent::FileDeleted {
            path: PathBuf::from("/test/deleted.txt"),
            deleted_by: identity.node_id(),
            timestamp: Utc::now(),
        };

        let mut signed_msg = SignedGossipMessage::new(event, &identity);

        // Tamper with the signature
        if !signed_msg.signature.is_empty() {
            signed_msg.signature[0] ^= 0xFF;
        }

        // Should fail verification
        assert!(signed_msg.verify().is_err());
    }

    #[test]
    fn test_signed_gossip_message_stale_detection() {
        let identity = Identity::generate();

        let event = DriveEvent::UserLeft {
            user: identity.node_id(),
            timestamp: Utc::now(),
        };

        let mut signed_msg = SignedGossipMessage::new(event, &identity);

        // Fresh message should not be stale
        assert!(!signed_msg.is_stale(MAX_MESSAGE_AGE_MS));

        // Make message old
        signed_msg.timestamp_ms -= MAX_MESSAGE_AGE_MS + 1000;

        // Now it should be stale
        assert!(signed_msg.is_stale(MAX_MESSAGE_AGE_MS));
    }

    #[tokio::test]
    async fn test_rate_limiter_many_peers() {
        let limiter = PeerRateLimiter::new(10, 1);

        // Simulate 100 different peers
        for i in 0..100 {
            let peer_id = format!("peer_{}", i);
            assert!(limiter.check(&peer_id).await);
        }

        {
            let limits = limiter.limits.lock().await;
            assert_eq!(limits.len(), 100);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_concurrent_access() {
        use std::sync::Arc;

        let limiter = Arc::new(PeerRateLimiter::new(1000, 1));
        let mut handles = Vec::new();

        for i in 0..10 {
            let limiter = limiter.clone();
            handles.push(tokio::spawn(async move {
                let peer_id = format!("peer_{}", i);
                for _ in 0..100 {
                    limiter.check(&peer_id).await;
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let limits = limiter.limits.lock().await;
        assert_eq!(limits.len(), 10);
    }
}
