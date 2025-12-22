use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct IdentityInfo {
    pub node_id: String,
    pub short_id: String,
}

/// Get the current node identity
#[tauri::command]
pub async fn get_identity(state: State<'_, AppState>) -> Result<IdentityInfo, String> {
    let node_id = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| "Identity not initialized".to_string())?;

    Ok(IdentityInfo {
        node_id: node_id.to_hex(),
        short_id: node_id.short_string(),
    })
}

/// Get P2P connection status
#[tauri::command]
pub async fn get_connection_status(state: State<'_, AppState>) -> Result<bool, String> {
    let endpoint_ready = state.endpoint.is_ready().await;
    Ok(endpoint_ready)
}
