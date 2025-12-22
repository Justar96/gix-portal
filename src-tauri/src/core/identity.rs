use crate::crypto::{Identity, NodeId};
use crate::storage::Database;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages the node's identity lifecycle
pub struct IdentityManager {
    identity: Arc<RwLock<Option<Identity>>>,
    db: Arc<Database>,
}

impl IdentityManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            identity: Arc::new(RwLock::new(None)),
            db,
        }
    }

    /// Load existing identity or generate new one on first run
    pub async fn initialize(&self) -> Result<NodeId> {
        let mut identity_guard = self.identity.write().await;

        // Try to load existing identity from database
        if let Some(bytes) = self.db.get_identity()? {
            let identity = Identity::from_bytes(&bytes)?;
            let node_id = identity.node_id();
            *identity_guard = Some(identity);
            tracing::info!("Loaded existing identity: {}", node_id);
            return Ok(node_id);
        }

        // Generate new identity on first run
        let identity = Identity::generate();
        let node_id = identity.node_id();

        // Persist to database
        self.db.save_identity(&identity.to_bytes())?;

        *identity_guard = Some(identity);
        tracing::info!("Generated new identity: {}", node_id);

        Ok(node_id)
    }

    /// Get the current NodeId (None if not initialized)
    pub async fn node_id(&self) -> Option<NodeId> {
        let guard = self.identity.read().await;
        guard.as_ref().map(|i| i.node_id())
    }

    /// Get the secret key bytes (for P2P endpoint initialization)
    pub async fn secret_key_bytes(&self) -> Option<[u8; 32]> {
        let guard = self.identity.read().await;
        guard.as_ref().map(|i| i.to_bytes())
    }

    /// Sign a message with our identity
    pub async fn sign(&self, message: &[u8]) -> Option<ed25519_dalek::Signature> {
        let guard = self.identity.read().await;
        guard.as_ref().map(|i| i.sign(message))
    }

    /// Check if identity is initialized
    pub async fn is_initialized(&self) -> bool {
        let guard = self.identity.read().await;
        guard.is_some()
    }
}
