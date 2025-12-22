use anyhow::Result;
use iroh::{endpoint::Connection, Endpoint, NodeId as IrohNodeId, SecretKey};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Application-level protocol name for P2P drive sharing
const ALPN: &[u8] = b"gix/1";

/// Manages the Iroh endpoint for P2P connections
pub struct P2PEndpoint {
    endpoint: Arc<RwLock<Option<Endpoint>>>,
    secret_key: SecretKey,
}

impl P2PEndpoint {
    /// Create a new P2P endpoint manager from Ed25519 secret key bytes
    pub fn new(secret_key_bytes: &[u8; 32]) -> Self {
        let secret_key = SecretKey::from_bytes(secret_key_bytes);
        Self {
            endpoint: Arc::new(RwLock::new(None)),
            secret_key,
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
}
