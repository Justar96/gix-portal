//! Event broadcasting via iroh-gossip protocol
//!
//! Provides pub/sub for real-time drive events between peers.
//! Each drive has its own gossip topic derived from its DriveId.

#![allow(dead_code)]

use crate::core::{DriveEvent, DriveEventDto, DriveId};
use anyhow::Result;
use iroh::Endpoint;
use iroh_gossip::net::Gossip;
use iroh_gossip::proto::TopicId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;

/// Manages gossip subscriptions per drive for real-time event broadcasting
pub struct EventBroadcaster {
    /// The gossip protocol instance
    gossip: Arc<Gossip>,
    /// Active topic subscriptions per drive
    subscriptions: RwLock<HashMap<DriveId, TopicSubscription>>,
    /// Channel to forward events to Tauri frontend
    frontend_tx: broadcast::Sender<DriveEventDto>,
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
    pub async fn new(endpoint: &Endpoint) -> Result<Self> {
        let gossip = Gossip::builder().spawn(endpoint.clone()).await?;

        // Create broadcast channel for frontend events (buffer 256 events)
        let (frontend_tx, _) = broadcast::channel(256);

        tracing::info!("EventBroadcaster initialized");

        Ok(Self {
            gossip: Arc::new(gossip),
            subscriptions: RwLock::new(HashMap::new()),
            frontend_tx,
        })
    }

    /// Subscribe to a drive's gossip topic
    ///
    /// This starts receiving events from other peers for the given drive.
    /// Events are automatically forwarded to the frontend channel.
    pub async fn subscribe(&self, drive_id: DriveId) -> Result<()> {
        let topic_id = self.drive_to_topic(&drive_id);

        // Check if already subscribed
        {
            let subs = self.subscriptions.read().await;
            if subs.contains_key(&drive_id) {
                tracing::debug!("Already subscribed to drive {}", drive_id);
                return Ok(());
            }
        }

        // Subscribe to the topic with no bootstrap peers initially
        // Peers will be added when we connect to them
        let topic = self.gossip.subscribe(topic_id, vec![])?;
        let (_sender, mut receiver) = topic.split();


        // Spawn receiver task to forward events to frontend
        let frontend_tx = self.frontend_tx.clone();
        let drive_id_hex = drive_id.to_hex();
        let drive_id_for_task = drive_id;

        let receiver_task = tokio::spawn(async move {
            use futures_lite::StreamExt;

            tracing::debug!("Started gossip receiver for drive {}", drive_id_hex);

            while let Some(event_result) = receiver.next().await {
                match event_result {
                    Ok(event) => {
                        use iroh_gossip::net::{Event, GossipEvent};

                        match event {
                            Event::Gossip(GossipEvent::Received(msg)) => {
                                // Deserialize the DriveEvent
                                match serde_json::from_slice::<DriveEvent>(&msg.content) {
                                    Ok(drive_event) => {
                                        let dto = DriveEventDto::from_event(
                                            &drive_id_for_task.to_hex(),
                                            &drive_event,
                                        );

                                        tracing::debug!(
                                            "Received gossip event: {} for drive {}",
                                            dto.event_type,
                                            drive_id_hex
                                        );

                                        // Forward to frontend (ignore if no receivers)
                                        let _ = frontend_tx.send(dto);
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
                                tracing::warn!(
                                    "Gossip receiver lagged for drive {}",
                                    drive_id_hex
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Gossip receiver error: {}", e);
                    }
                }
            }

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
    pub async fn broadcast(&self, drive_id: &DriveId, event: DriveEvent) -> Result<()> {
        let topic_id = self.drive_to_topic(drive_id);

        // Serialize the event
        let data = serde_json::to_vec(&event)?;

        // Get a sender for this topic
        let topic = self.gossip.subscribe(topic_id, vec![])?;
        let (sender, _receiver) = topic.split();

        // Broadcast the message
        sender.broadcast(data.into()).await?;

        tracing::debug!(
            "Broadcast {} event for drive {}",
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

    /// Get the underlying gossip instance for advanced use
    pub fn gossip(&self) -> Arc<Gossip> {
        self.gossip.clone()
    }
}

impl Drop for EventBroadcaster {
    fn drop(&mut self) {
        // Subscriptions will be cleaned up when tasks are dropped
        tracing::debug!("EventBroadcaster dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
