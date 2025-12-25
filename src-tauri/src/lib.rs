mod commands;
mod core;
mod crypto;
mod network;
mod state;
mod storage;
mod tray;

use commands::{
    accept_invite, acquire_lock, cancel_transfer, check_permission, create_drive, delete_drive,
    delete_path, dismiss_conflict, download_file, extend_lock, force_release_lock, generate_invite,
    get_audit_count, get_audit_log, get_conflict, get_conflict_count, get_connection_status,
    get_denied_access_log, get_drive, get_drive_audit_log, get_identity, get_lock_status,
    get_online_count, get_online_users, get_recent_activity, get_sync_status, get_transfer,
    grant_permission, is_watching, join_drive_presence, leave_drive_presence, list_conflicts,
    list_drives, list_files, list_locks, list_permissions, list_revoked_tokens, list_transfers,
    presence_heartbeat, read_file, read_file_encrypted, release_lock, rename_drive, rename_path,
    resolve_conflict, revoke_invite, revoke_permission, start_sync, start_watching, stop_sync,
    stop_watching, subscribe_drive_events, upload_file, verify_invite, write_file,
    write_file_encrypted, SecurityStore,
};
use core::{
    AuditLogger, ConflictManager, DriveEvent, DriveEventDto, DriveId, LockManager, PresenceManager,
    RateLimiter, SharedRateLimiter,
};
use state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tokio::sync::broadcast;

use crate::network::SyncEngine;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,gix=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Gix P2P Drive Share");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Show window when another instance is launched
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Initialize system tray
            if let Err(e) = tray::init(app) {
                tracing::error!("Failed to initialize system tray: {}", e);
            }

            let app_handle = app.handle().clone();

            // Get data directory - use match instead of expect for production safety
            let data_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    tracing::error!("Failed to get app data directory: {}", e);
                    // Use a fallback directory in temp
                    std::env::temp_dir().join("gix-portal")
                }
            };

            tracing::info!("Data directory: {:?}", data_dir);

            // Initialize state synchronously to ensure it's available before any commands run
            // Using block_on since we're not in an async context but need to await the initialization
            let state =
                tauri::async_runtime::block_on(async { AppState::initialize(data_dir).await });

            match state {
                Ok(state) => {
                    // Spawn event forwarding task if event_broadcaster is available
                    if let Some(ref broadcaster) = state.event_broadcaster {
                        let event_rx = broadcaster.subscribe_frontend();
                        let app_handle_for_events = app_handle.clone();

                        tauri::async_runtime::spawn(async move {
                            spawn_event_forwarder(app_handle_for_events, event_rx).await;
                        });
                    }

                    // Spawn file watcher event forwarding task
                    if let (Some(ref watcher), Some(ref sync_engine)) =
                        (&state.file_watcher, &state.sync_engine)
                    {
                        let watcher_rx = watcher.subscribe();
                        let sync_engine_clone = sync_engine.clone();
                        let app_handle_for_watcher = app_handle.clone();

                        tauri::async_runtime::spawn(async move {
                            spawn_watcher_forwarder(
                                app_handle_for_watcher,
                                watcher_rx,
                                sync_engine_clone,
                            )
                            .await;
                        });
                    }

                    // Get node ID for managers - handle gracefully if not available
                    let node_id = tauri::async_runtime::block_on(async {
                        state.identity_manager.node_id().await
                    });

                    let node_id = match node_id {
                        Some(id) => id,
                        None => {
                            tracing::error!("Node ID not available during initialization");
                            // Still manage the state so basic operations work
                            app_handle.manage(state);
                            return Ok(());
                        }
                    };

                    // Initialize SecurityStore for Phase 3 with database persistence
                    let security_store = Arc::new(SecurityStore::new(state.db.clone()));
                    // Load persisted ACLs from database
                    if let Err(e) = security_store.load_from_db() {
                        tracing::error!("Failed to load security data from database: {}", e);
                    }
                    app_handle.manage(security_store.clone());

                    // Initialize AuditLogger for security event tracking
                    let audit_logger = Arc::new(AuditLogger::new(state.db.clone()));
                    app_handle.manage(audit_logger);
                    tracing::info!("AuditLogger initialized for security event tracking");

                    // Configure ACL checker for gossip sender authorization
                    if let Some(ref broadcaster) = state.event_broadcaster {
                        let security_for_acl = security_store.clone();
                        let acl_checker: network::AclChecker =
                            Arc::new(move |drive_id, sender_id| {
                                // Check if sender has at least Read permission on the drive
                                // Use block_in_place to properly block within tokio runtime context
                                // This moves the current thread out of the worker pool during the blocking call
                                let acl = tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(
                                        security_for_acl.get_or_create_acl(drive_id, ""),
                                    )
                                });
                                use crate::crypto::Permission;
                                acl.check_permission(sender_id, "/", Permission::Read)
                            });

                        // Set the ACL checker asynchronously
                        let broadcaster_clone = broadcaster.clone();
                        tauri::async_runtime::spawn(async move {
                            broadcaster_clone.set_acl_checker(acl_checker).await;
                        });
                    }

                    // Initialize rate limiter for abuse prevention
                    let rate_limiter: SharedRateLimiter = Arc::new(RateLimiter::new());
                    app_handle.manage(rate_limiter);
                    tracing::info!("Rate limiter initialized");

                    // Initialize LockManager for Phase 4
                    let lock_manager = Arc::new(LockManager::new(node_id));
                    app_handle.manage(lock_manager.clone());

                    // Initialize ConflictManager for Phase 4
                    let conflict_manager = Arc::new(ConflictManager::new());
                    app_handle.manage(conflict_manager.clone());

                    // Initialize PresenceManager for Phase 4
                    let presence_manager = Arc::new(PresenceManager::new(node_id));
                    app_handle.manage(presence_manager.clone());

                    // Start cleanup manager for resource maintenance
                    let cleanup_manager = core::CleanupManager::new();
                    let _cleanup_handle = cleanup_manager.start(
                        lock_manager,
                        conflict_manager,
                        presence_manager,
                        security_store,
                    );
                    tracing::info!("Cleanup manager started");

                    // Register EncryptionManager for E2E encryption commands
                    if let Some(ref em) = state.encryption_manager {
                        app_handle.manage(em.clone());
                        tracing::info!("EncryptionManager registered with Tauri");

                        // SECURITY: Set up window blur listener to clear encryption key cache
                        // This protects against cold boot attacks if device is stolen while app is running
                        let em_for_blur = em.clone();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.on_window_event(move |event| {
                                if let tauri::WindowEvent::Focused(false) = event {
                                    // Window lost focus - clear encryption key cache for security
                                    let em_clone = em_for_blur.clone();
                                    tauri::async_runtime::spawn(async move {
                                        em_clone.clear_cache().await;
                                        tracing::debug!(
                                            "Encryption key cache cleared due to window blur"
                                        );
                                    });
                                }
                            });
                            tracing::info!(
                                "Window blur listener configured for encryption cache clearing"
                            );
                        }
                    }

                    app_handle.manage(state);
                    tracing::info!("Application state initialized successfully");
                }
                Err(e) => {
                    tracing::error!("Failed to initialize application: {}", e);
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to initialize application: {}", e),
                    )) as Box<dyn std::error::Error>);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_identity,
            get_connection_status,
            create_drive,
            delete_drive,
            rename_drive,
            list_drives,
            get_drive,
            list_files,
            read_file,
            write_file,
            read_file_encrypted,
            write_file_encrypted,
            delete_path,
            rename_path,
            // Phase 2: Sync commands
            start_sync,
            stop_sync,
            get_sync_status,
            subscribe_drive_events,
            // Phase 2: File watcher commands
            start_watching,
            stop_watching,
            is_watching,
            // Phase 2: File transfer commands
            upload_file,
            download_file,
            list_transfers,
            get_transfer,
            cancel_transfer,
            // Phase 3: Security commands
            generate_invite,
            verify_invite,
            accept_invite,
            revoke_invite,
            list_revoked_tokens,
            list_permissions,
            grant_permission,
            revoke_permission,
            check_permission,
            // Phase 4: Locking commands
            acquire_lock,
            release_lock,
            get_lock_status,
            list_locks,
            extend_lock,
            force_release_lock,
            // Phase 4: Conflict commands
            list_conflicts,
            get_conflict,
            resolve_conflict,
            get_conflict_count,
            dismiss_conflict,
            // Phase 4: Presence commands
            get_online_users,
            get_online_count,
            get_recent_activity,
            join_drive_presence,
            leave_drive_presence,
            presence_heartbeat,
            // Security: Audit logging commands
            get_audit_log,
            get_audit_count,
            get_drive_audit_log,
            get_denied_access_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Handle app lifecycle events for graceful shutdown
            match event {
                RunEvent::ExitRequested { api, .. } => {
                    // This is called when exit is requested but before actual exit
                    // We can't prevent exit here, but we can initiate shutdown early
                    tracing::info!("Application exit requested, initiating graceful shutdown...");

                    // Get app state and perform graceful shutdown
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        // Use block_on to run the async shutdown within the event handler
                        // This ensures all async resources are cleaned up before the runtime is destroyed
                        tauri::async_runtime::block_on(async {
                            state.shutdown().await;
                        });
                    }

                    tracing::info!("Graceful shutdown complete");
                    // Don't prevent exit
                    let _ = api;
                }
                RunEvent::Exit => {
                    // Exit is already happening, shutdown should have already run
                    tracing::debug!("Application exiting");
                }
                _ => {}
            }
        });
}

/// Spawns a background task that forwards drive events to the frontend
async fn spawn_event_forwarder(
    app_handle: AppHandle,
    mut event_rx: broadcast::Receiver<DriveEventDto>,
) {
    tracing::info!("Event forwarder started");

    loop {
        match event_rx.recv().await {
            Ok(event) => {
                // Emit event to frontend
                if let Err(e) = app_handle.emit("drive-event", &event) {
                    tracing::warn!("Failed to emit drive event: {}", e);
                }
            }
            Err(broadcast::error::RecvError::Lagged(count)) => {
                tracing::warn!("Event receiver lagged, missed {} events", count);
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("Event channel closed, stopping forwarder");
                break;
            }
        }
    }
}

/// Spawns a background task that forwards file watcher events to SyncEngine and frontend
async fn spawn_watcher_forwarder(
    app_handle: AppHandle,
    mut watcher_rx: broadcast::Receiver<(DriveId, DriveEvent)>,
    sync_engine: Arc<SyncEngine>,
) {
    tracing::info!("File watcher forwarder started");

    loop {
        match watcher_rx.recv().await {
            Ok((drive_id, event)) => {
                // Forward to sync engine for processing (metadata updates, gossip broadcast)
                if let Err(e) = sync_engine.on_local_change(&drive_id, event.clone()).await {
                    tracing::warn!("Failed to process local change: {}", e);
                }

                // Also emit directly to frontend for immediate UI update
                let dto = DriveEventDto::from_event(&hex::encode(drive_id.as_bytes()), &event);
                if let Err(e) = app_handle.emit("drive-event", &dto) {
                    tracing::warn!("Failed to emit watcher event: {}", e);
                }
            }
            Err(broadcast::error::RecvError::Lagged(count)) => {
                tracing::warn!("Watcher receiver lagged, missed {} events", count);
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("Watcher channel closed, stopping forwarder");
                break;
            }
        }
    }
}
