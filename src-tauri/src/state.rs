use crate::core::{FileWatcherManager, IdentityManager, SharedDrive};
use crate::crypto::EncryptionManager;
use crate::network::{DocsManager, EventBroadcaster, FileTransferManager, P2PEndpoint, SyncEngine};
use crate::storage::Database;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Application-wide state managed by Tauri
pub struct AppState {
    /// Database for persistent storage
    pub db: Arc<Database>,
    /// Identity manager for node identity
    pub identity_manager: Arc<IdentityManager>,
    /// P2P endpoint for networking
    pub endpoint: Arc<P2PEndpoint>,
    /// In-memory cache of drives (keyed by DriveId bytes)
    pub drives: Arc<RwLock<HashMap<[u8; 32], SharedDrive>>>,
    /// Encryption manager for E2E file encryption
    pub encryption_manager: Option<Arc<EncryptionManager>>,

    // Phase 2 components
    /// Sync engine for coordinating real-time sync
    pub sync_engine: Option<Arc<SyncEngine>>,
    /// Event broadcaster for gossip pub/sub
    pub event_broadcaster: Option<Arc<EventBroadcaster>>,
    /// Document manager for CRDT metadata sync (used by SyncEngine)
    #[allow(dead_code)]
    pub docs_manager: Option<Arc<DocsManager>>,
    /// File watcher manager for detecting local changes
    pub file_watcher: Option<Arc<FileWatcherManager>>,
    /// File transfer manager for blob sync
    pub file_transfer: Option<Arc<FileTransferManager>>,
}

impl AppState {
    /// Initialize application state
    pub async fn initialize(data_dir: PathBuf) -> anyhow::Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)?;
        tracing::info!("Using data directory: {:?}", data_dir);

        // Open database
        let db_path = data_dir.join("gix.redb");
        let db = Arc::new(Database::open(&db_path)?);
        tracing::info!("Database opened at: {:?}", db_path);

        // Initialize identity manager and load/generate identity
        let identity_manager = Arc::new(IdentityManager::new(db.clone()));
        let node_id = identity_manager.initialize().await?;
        tracing::info!("Node identity: {}", node_id);

        // Get secret key bytes for P2P endpoint
        let secret_key_bytes = identity_manager
            .secret_key_bytes()
            .await
            .ok_or_else(|| anyhow::anyhow!("Identity not initialized after initialization"))?;

        // Initialize P2P endpoint
        let endpoint = Arc::new(P2PEndpoint::new(&secret_key_bytes));
        endpoint.start().await?;
        tracing::info!("P2P endpoint started");

        // Load existing drives from database into memory
        let drives = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut drives_guard = drives.write().await;
            for (id, data) in db.list_drives()? {
                match serde_json::from_slice::<SharedDrive>(&data) {
                    Ok(drive) => {
                        tracing::debug!("Loaded drive: {} ({})", drive.name, drive.id);
                        drives_guard.insert(id, drive);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to deserialize drive: {}", e);
                    }
                }
            }
            tracing::info!("Loaded {} drives from database", drives_guard.len());
        }

        // Initialize Phase 2 components (gossip, docs, sync, watcher, transfer)
        let (sync_engine, event_broadcaster, docs_manager, file_watcher, file_transfer) =
            Self::initialize_sync_components(&endpoint, &identity_manager, &data_dir, db.clone())
                .await;

        // Initialize EncryptionManager for E2E file encryption
        let encryption_manager = match EncryptionManager::new(db.clone()) {
            Ok(em) => {
                tracing::info!("EncryptionManager initialized");
                Some(Arc::new(em))
            }
            Err(e) => {
                tracing::error!("Failed to initialize EncryptionManager: {}", e);
                None
            }
        };

        Ok(Self {
            db,
            identity_manager,
            endpoint,
            drives,
            encryption_manager,
            sync_engine,
            event_broadcaster,
            docs_manager,
            file_watcher,
            file_transfer,
        })
    }

    /// Initialize Phase 2 sync components
    ///
    /// Returns (sync_engine, event_broadcaster, docs_manager, file_watcher, file_transfer) wrapped in Option.
    /// If initialization fails, logs error and returns None for all.
    async fn initialize_sync_components(
        endpoint: &Arc<P2PEndpoint>,
        identity_manager: &Arc<IdentityManager>,
        data_dir: &std::path::Path,
        db: Arc<Database>,
    ) -> (
        Option<Arc<SyncEngine>>,
        Option<Arc<EventBroadcaster>>,
        Option<Arc<DocsManager>>,
        Option<Arc<FileWatcherManager>>,
        Option<Arc<FileTransferManager>>,
    ) {
        // Get the underlying Iroh endpoint
        let iroh_endpoint = match endpoint.get_endpoint().await {
            Some(ep) => ep,
            None => {
                tracing::warn!("Cannot initialize sync: P2P endpoint not ready");
                return (None, None, None, None, None);
            }
        };

        // Get node ID for event attribution
        let node_id = match identity_manager.node_id().await {
            Some(id) => id,
            None => {
                tracing::warn!("Cannot initialize sync: node ID not available");
                return (None, None, None, None, None);
            }
        };

        // Get identity for signing gossip messages
        let identity = match identity_manager.get_identity().await {
            Some(id) => id,
            None => {
                tracing::warn!("Cannot initialize sync: identity not available for signing");
                return (None, None, None, None, None);
            }
        };

        // Initialize EventBroadcaster with identity for message signing
        let event_broadcaster = match EventBroadcaster::new(&iroh_endpoint, identity).await {
            Ok(eb) => Arc::new(eb),
            Err(e) => {
                tracing::error!("Failed to initialize EventBroadcaster: {}", e);
                return (None, None, None, None, None);
            }
        };

        // Initialize FileWatcherManager
        let file_watcher = {
            let watcher = FileWatcherManager::new(node_id);
            tracing::info!("FileWatcherManager initialized");
            Some(Arc::new(watcher))
        };

        // Initialize FileTransferManager
        let file_transfer = match FileTransferManager::new(&iroh_endpoint, data_dir, node_id).await
        {
            Ok(ftm) => {
                tracing::info!("FileTransferManager initialized");
                Some(Arc::new(ftm))
            }
            Err(e) => {
                tracing::error!("Failed to initialize FileTransferManager: {}", e);
                None
            }
        };

        // Initialize DocsManager
        let docs_manager = match (event_broadcaster.gossip().await, file_transfer.as_ref()) {
            (Some(gossip), Some(transfer)) => {
                match DocsManager::new(data_dir, db, transfer.blobs(), gossip).await {
                    Ok(dm) => Some(Arc::new(dm)),
                    Err(e) => {
                        tracing::error!("Failed to initialize DocsManager: {}", e);
                        None
                    }
                }
            }
            _ => {
                tracing::warn!("DocsManager unavailable: gossip or blobs not initialized");
                None
            }
        };

        // Initialize SyncEngine
        let sync_engine = docs_manager
            .as_ref()
            .map(|dm| Arc::new(SyncEngine::new(dm.clone(), event_broadcaster.clone())));

        tracing::info!("Phase 2 sync components initialized successfully");

        (
            sync_engine,
            Some(event_broadcaster),
            docs_manager,
            file_watcher,
            file_transfer,
        )
    }

    /// Gracefully shutdown all async components
    ///
    /// This must be called before the Tokio runtime is destroyed to avoid
    /// panics from async Drop implementations in iroh-gossip and other async libs.
    pub async fn shutdown(&self) {
        tracing::info!("AppState shutting down...");

        // Shutdown event broadcaster first (stops gossip tasks)
        if let Some(ref broadcaster) = self.event_broadcaster {
            broadcaster.shutdown().await;
        }

        // Shutdown P2P endpoint
        self.endpoint.shutdown().await;

        tracing::info!("AppState shutdown complete");
    }
}
