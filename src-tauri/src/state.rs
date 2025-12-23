use crate::core::{IdentityManager, SharedDrive};
use crate::network::{DocsManager, EventBroadcaster, P2PEndpoint, SyncEngine};
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

    // Phase 2 components
    /// Sync engine for coordinating real-time sync
    pub sync_engine: Option<Arc<SyncEngine>>,
    /// Event broadcaster for gossip pub/sub
    pub event_broadcaster: Option<Arc<EventBroadcaster>>,
    /// Document manager for CRDT metadata sync
    pub docs_manager: Option<Arc<DocsManager>>,
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
            .expect("Identity should be initialized");

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

        // Initialize Phase 2 components (gossip, docs, sync)
        let (sync_engine, event_broadcaster, docs_manager) =
            Self::initialize_sync_components(&endpoint, &data_dir).await;

        Ok(Self {
            db,
            identity_manager,
            endpoint,
            drives,
            sync_engine,
            event_broadcaster,
            docs_manager,
        })
    }

    /// Initialize Phase 2 sync components
    ///
    /// Returns (sync_engine, event_broadcaster, docs_manager) wrapped in Option.
    /// If initialization fails, logs error and returns None for all.
    async fn initialize_sync_components(
        endpoint: &Arc<P2PEndpoint>,
        data_dir: &PathBuf,
    ) -> (
        Option<Arc<SyncEngine>>,
        Option<Arc<EventBroadcaster>>,
        Option<Arc<DocsManager>>,
    ) {
        // Get the underlying Iroh endpoint
        let iroh_endpoint = match endpoint.get_endpoint().await {
            Some(ep) => ep,
            None => {
                tracing::warn!("Cannot initialize sync: P2P endpoint not ready");
                return (None, None, None);
            }
        };

        // Initialize EventBroadcaster
        let event_broadcaster = match EventBroadcaster::new(&iroh_endpoint).await {
            Ok(eb) => Arc::new(eb),
            Err(e) => {
                tracing::error!("Failed to initialize EventBroadcaster: {}", e);
                return (None, None, None);
            }
        };

        // Initialize DocsManager
        let docs_manager = match DocsManager::new(data_dir).await {
            Ok(dm) => Arc::new(dm),
            Err(e) => {
                tracing::error!("Failed to initialize DocsManager: {}", e);
                return (None, Some(event_broadcaster), None);
            }
        };

        // Initialize SyncEngine
        let sync_engine = Arc::new(SyncEngine::new(
            docs_manager.clone(),
            event_broadcaster.clone(),
        ));

        tracing::info!("Phase 2 sync components initialized successfully");

        (Some(sync_engine), Some(event_broadcaster), Some(docs_manager))
    }
}
