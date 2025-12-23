//! P2P endpoint management for Iroh networking
//!
//! Many methods are scaffolding for future P2P features.

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use iroh::{endpoint::Connection, Endpoint, NodeId as IrohNodeId, SecretKey};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Application-level protocol name for P2P drive sharing
const ALPN: &[u8] = b"gix/1";

/// Information about a connected peer
#[derive(Clone, Debug, Serialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub connected_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// Connection status information for the frontend
#[derive(Clone, Debug, Serialize)]
pub struct ConnectionInfo {
    pub is_online: bool,
    pub node_id: Option<String>,
    pub relay_url: Option<String>,
    pub peer_count: usize,
}

/// Manages the Iroh endpoint for P2P connections
pub struct P2PEndpoint {
    endpoint: Arc<RwLock<Option<Endpoint>>>,
    secret_key: SecretKey,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
}

impl P2PEndpoint {
    /// Create a new P2P endpoint manager from Ed25519 secret key bytes
    pub fn new(secret_key_bytes: &[u8; 32]) -> Self {
        let secret_key = SecretKey::from_bytes(secret_key_bytes);
        Self {
            endpoint: Arc::new(RwLock::new(None)),
            secret_key,
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize and bind the endpoint
    pub async fn start(&self) -> Result<()> {
        let endpoint = Endpoint::builder()
            .secret_key(self.secret_key.clone())
            .alpns(vec![ALPN.to_vec()])
            // Use n0's discovery network for NAT traversal
            .discovery_n0()
            .bind()
            .await?;

        let node_id = endpoint.node_id();
        tracing::info!("Iroh endpoint started with NodeId: {}", node_id);

        // Log relay information if available
        let home_relay = endpoint.home_relay();
        tracing::info!("Home relay: {:?}", home_relay);

        let mut guard = self.endpoint.write().await;
        *guard = Some(endpoint);

        Ok(())
    }

    /// Get the Iroh NodeId
    pub async fn node_id(&self) -> Option<IrohNodeId> {
        let guard = self.endpoint.read().await;
        guard.as_ref().map(|e| e.node_id())
    }

    /// Check if endpoint is ready
    pub async fn is_ready(&self) -> bool {
        let guard = self.endpoint.read().await;
        guard.is_some()
    }

    /// Get comprehensive connection information
    pub async fn get_connection_info(&self) -> ConnectionInfo {
        let guard = self.endpoint.read().await;
        let peers = self.peers.read().await;

        match guard.as_ref() {
            Some(endpoint) => {
                // TODO: Properly extract relay URL from Watcher
                // For now, we skip this as the Watcher API is complex
                let relay_url: Option<String> = None;

                ConnectionInfo {
                    is_online: true,
                    node_id: Some(endpoint.node_id().to_string()),
                    relay_url,
                    peer_count: peers.len(),
                }
            }
            None => ConnectionInfo {
                is_online: false,
                node_id: None,
                relay_url: None,
                peer_count: 0,
            },
        }
    }

    /// Get list of connected peers
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Track a new peer connection
    pub async fn add_peer(&self, node_id: IrohNodeId) {
        let now = Utc::now();
        let peer_info = PeerInfo {
            node_id: node_id.to_string(),
            connected_at: now,
            last_seen: now,
        };
        let mut peers = self.peers.write().await;
        peers.insert(node_id.to_string(), peer_info);
        tracing::info!("Peer added: {}", node_id);
    }

    /// Remove a peer from tracking
    pub async fn remove_peer(&self, node_id: &IrohNodeId) {
        let mut peers = self.peers.write().await;
        peers.remove(&node_id.to_string());
        tracing::info!("Peer removed: {}", node_id);
    }

    /// Connect to a peer by their NodeId
    pub async fn connect(&self, peer_id: IrohNodeId) -> Result<Connection> {
        let guard = self.endpoint.read().await;
        let endpoint = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Endpoint not initialized"))?;

        let conn = endpoint.connect(peer_id, ALPN).await?;
        tracing::info!("Connected to peer: {}", peer_id);
        Ok(conn)
    }

    /// Accept incoming connections (call in a loop)
    pub async fn accept(&self) -> Option<Connection> {
        let guard = self.endpoint.read().await;
        let endpoint = guard.as_ref()?;

        match endpoint.accept().await {
            Some(incoming) => match incoming.await {
                Ok(conn) => {
                    tracing::info!("Accepted connection from: {:?}", conn.remote_node_id());
                    Some(conn)
                }
                Err(e) => {
                    tracing::warn!("Failed to accept connection: {}", e);
                    None
                }
            },
            None => None,
        }
    }

    /// Shutdown the endpoint gracefully
    pub async fn shutdown(&self) {
        let mut guard = self.endpoint.write().await;
        if let Some(endpoint) = guard.take() {
            endpoint.close().await;
            tracing::info!("Endpoint shutdown complete");
        }
    }

    /// Get the underlying Iroh Endpoint for use with gossip/docs protocols
    ///
    /// Returns None if the endpoint hasn't been started yet.
    pub async fn get_endpoint(&self) -> Option<Endpoint> {
        let guard = self.endpoint.read().await;
        guard.clone()
    }
}
