mod commands;
mod core;
mod crypto;
mod network;
mod state;
mod storage;

use commands::{create_drive, get_connection_status, get_drive, get_identity, list_drives, list_files};
use state::AppState;
use tauri::Manager;
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
            list_drives,
            get_drive,
            list_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
