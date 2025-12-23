mod commands;
mod core;
mod crypto;
mod network;
mod state;
mod storage;

use commands::{
    create_drive, delete_drive, get_connection_status, get_drive, get_identity, get_sync_status,
    list_drives, list_files, rename_drive, start_sync, stop_sync, subscribe_drive_events,
};
use core::DriveEventDto;
use state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast;
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
        .setup(|app| {
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
