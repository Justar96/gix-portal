mod commands;
mod core;
mod crypto;
mod network;
mod state;
mod storage;
mod tray;

use commands::{
    acquire_lock, cancel_transfer, check_permission, create_drive, delete_drive, dismiss_conflict,
    download_file, extend_lock, force_release_lock, generate_invite, get_conflict,
    get_conflict_count, get_connection_status, get_drive, get_identity, get_lock_status,
    get_online_count, get_online_users, get_recent_activity, get_sync_status, get_transfer,
    grant_permission, is_watching, join_drive_presence, leave_drive_presence, list_conflicts,
    list_drives, list_files, list_locks, list_permissions, list_transfers, presence_heartbeat,
    release_lock, rename_drive, resolve_conflict, revoke_permission, start_sync, start_watching,
    stop_sync, stop_watching, subscribe_drive_events, upload_file, verify_invite, SecurityStore,
};
use core::{ConflictManager, DriveEvent, DriveEventDto, DriveId, LockManager, PresenceManager};
use state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
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
        .setup(|app| {
            // Initialize system tray
            if let Err(e) = tray::init(app) {
                tracing::error!("Failed to initialize system tray: {}", e);
            }

            let app_handle = app.handle().clone();

            // Get data directory
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            tracing::info!("Data directory: {:?}", data_dir);

            // Initialize state asynchronously
            tauri::async_runtime::spawn(async move {
                match AppState::initialize(data_dir).await {
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

                        // Initialize SecurityStore for Phase 3
                        let security_store = Arc::new(SecurityStore::new());
                        app_handle.manage(security_store);

                        // Initialize LockManager for Phase 4
                        let node_id = state
                            .identity_manager
                            .node_id()
                            .await
                            .expect("Node ID should be available");
                        let lock_manager = Arc::new(LockManager::new(node_id));
                        app_handle.manage(lock_manager);

                        // Initialize ConflictManager for Phase 4
                        let conflict_manager = Arc::new(ConflictManager::new());
                        app_handle.manage(conflict_manager);

                        // Initialize PresenceManager for Phase 4
                        let presence_manager = Arc::new(PresenceManager::new(node_id));
                        app_handle.manage(presence_manager);

                        app_handle.manage(state);
                        tracing::info!("Application state initialized successfully");
                    }
                    Err(e) => {
                        tracing::error!("Failed to initialize application: {}", e);
                    }
                }
            });

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
