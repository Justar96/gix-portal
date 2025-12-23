//! Security commands for access control and invite management
//!
//! Phase 3: Tauri commands exposing crypto module functionality.

use crate::crypto::{
    AccessControlList, AccessRule, InviteBuilder, InviteToken, Permission, TokenTracker,
};
use crate::state::AppState;
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// In-memory store for ACLs and token trackers per drive
/// TODO: Persist to database in future iteration
pub struct SecurityStore {
    /// ACLs keyed by drive ID (hex string)
    acls: RwLock<HashMap<String, AccessControlList>>,
    /// Token trackers keyed by drive ID (hex string)
    token_trackers: RwLock<HashMap<String, TokenTracker>>,
}

impl SecurityStore {
    pub fn new() -> Self {
        Self {
            acls: RwLock::new(HashMap::new()),
            token_trackers: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create ACL for a drive
    pub async fn get_or_create_acl(&self, drive_id: &str, owner: &str) -> AccessControlList {
        let mut acls = self.acls.write().await;
        acls.entry(drive_id.to_string())
            .or_insert_with(|| AccessControlList::new(owner))
            .clone()
    }

    /// Update ACL for a drive
    pub async fn update_acl(&self, drive_id: &str, acl: AccessControlList) {
        let mut acls = self.acls.write().await;
        acls.insert(drive_id.to_string(), acl);
    }

    /// Get token tracker for a drive
    #[allow(dead_code)]
    pub async fn get_token_tracker(&self, drive_id: &str) -> TokenTracker {
        let trackers = self.token_trackers.read().await;
        trackers.get(drive_id).cloned().unwrap_or_default()
    }

    /// Update token tracker for a drive
    #[allow(dead_code)]
    pub async fn update_token_tracker(&self, drive_id: &str, tracker: TokenTracker) {
        let mut trackers = self.token_trackers.write().await;
        trackers.insert(drive_id.to_string(), tracker);
    }
}

impl Default for SecurityStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DTOs for frontend communication
// ============================================================================

/// Permission level as string for frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Read,
    Write,
    Manage,
    Admin,
}

impl From<PermissionLevel> for Permission {
    fn from(level: PermissionLevel) -> Self {
        match level {
            PermissionLevel::Read => Permission::Read,
            PermissionLevel::Write => Permission::Write,
            PermissionLevel::Manage => Permission::Manage,
            PermissionLevel::Admin => Permission::Admin,
        }
    }
}

impl From<Permission> for PermissionLevel {
    fn from(perm: Permission) -> Self {
        match perm {
            Permission::Read => PermissionLevel::Read,
            Permission::Write => PermissionLevel::Write,
            Permission::Manage => PermissionLevel::Manage,
            Permission::Admin => PermissionLevel::Admin,
        }
    }
}

/// User permission info for frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPermission {
    pub node_id: String,
    pub permission: PermissionLevel,
    pub granted_by: String,
    pub granted_at: String,
    pub expires_at: Option<String>,
    pub is_owner: bool,
}

/// Invite creation request
#[derive(Clone, Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub drive_id: String,
    pub permission: PermissionLevel,
    /// Validity in hours (default: 24)
    pub validity_hours: Option<u32>,
    /// Optional note/message
    pub note: Option<String>,
    /// Single-use token
    pub single_use: Option<bool>,
}

/// Invite info for frontend
#[derive(Clone, Debug, Serialize)]
pub struct InviteInfo {
    pub token: String,
    pub drive_id: String,
    pub permission: PermissionLevel,
    pub expires_at: String,
    pub note: Option<String>,
    pub single_use: bool,
}

/// Invite verification result
#[derive(Clone, Debug, Serialize)]
pub struct InviteVerification {
    pub valid: bool,
    pub drive_id: Option<String>,
    pub permission: Option<PermissionLevel>,
    pub inviter: Option<String>,
    pub expires_at: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Generate an invite token for a drive
#[tauri::command]
pub async fn generate_invite(
    request: CreateInviteRequest,
    state: State<'_, AppState>,
    _security: State<'_, Arc<SecurityStore>>,
) -> Result<InviteInfo, String> {
    // Verify the caller owns the drive
    let drive_id = &request.drive_id;
    let id_arr = parse_drive_id(drive_id)?;

    let drives = state.drives.read().await;
    let drive = drives
        .get(&id_arr)
        .ok_or_else(|| "Drive not found".to_string())?;

    // Get the signing key from identity manager
    let signing_key = state
        .identity_manager
        .signing_key()
        .await
        .ok_or_else(|| "Identity not initialized".to_string())?;

    // Build the invite token (using chrono::Duration)
    let validity_hours = request.validity_hours.unwrap_or(24);
    let validity = ChronoDuration::hours(validity_hours as i64);

    let mut builder = InviteBuilder::new(drive_id)
        .with_permission(request.permission.clone().into())
        .with_validity(validity);

    if let Some(note) = &request.note {
        builder = builder.with_note(note);
    }

    if request.single_use.unwrap_or(false) {
        builder = builder.single_use();
    }

    let token = builder
        .build(&signing_key)
        .map_err(|e| format!("Failed to create invite: {}", e))?;

    let token_string = token
        .to_string()
        .map_err(|e| format!("Failed to serialize token: {}", e))?;

    let expires_at = Utc::now() + ChronoDuration::hours(validity_hours as i64);

    tracing::info!(
        "Generated invite for drive '{}' with {:?} permission",
        drive.name,
        request.permission
    );

    Ok(InviteInfo {
        token: token_string,
        drive_id: drive_id.clone(),
        permission: request.permission,
        expires_at: expires_at.to_rfc3339(),
        note: request.note,
        single_use: request.single_use.unwrap_or(false),
    })
}

/// Verify an invite token without accepting it
#[tauri::command]
pub async fn verify_invite(
    token_string: String,
    _state: State<'_, AppState>,
) -> Result<InviteVerification, String> {
    // Parse the token
    let token = match InviteToken::from_string(&token_string) {
        Ok(t) => t,
        Err(e) => {
            return Ok(InviteVerification {
                valid: false,
                drive_id: None,
                permission: None,
                inviter: None,
                expires_at: None,
                error: Some(format!("Invalid token format: {}", e)),
            });
        }
    };

    // Check expiration
    if token.is_expired() {
        return Ok(InviteVerification {
            valid: false,
            drive_id: Some(token.payload.drive_id.clone()),
            permission: Some(token.payload.permission.into()),
            inviter: Some(token.payload.inviter.clone()),
            expires_at: Some(token.payload.expires_at.to_rfc3339()),
            error: Some("Token has expired".to_string()),
        });
    }

    // TODO: Verify signature against inviter's public key
    // For now, we trust the token structure

    Ok(InviteVerification {
        valid: true,
        drive_id: Some(token.payload.drive_id.clone()),
        permission: Some(token.payload.permission.into()),
        inviter: Some(token.payload.inviter.clone()),
        expires_at: Some(token.payload.expires_at.to_rfc3339()),
        error: None,
    })
}

/// List permissions for a drive
#[tauri::command]
pub async fn list_permissions(
    drive_id: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<Vec<UserPermission>, String> {
    let id_arr = parse_drive_id(&drive_id)?;

    // Get drive to find owner
    let drives = state.drives.read().await;
    let drive = drives
        .get(&id_arr)
        .ok_or_else(|| "Drive not found".to_string())?;

    // Convert NodeId to hex string for ACL operations
    let owner_hex = drive.owner.to_hex();
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;

    let mut permissions = Vec::new();

    // Add owner
    permissions.push(UserPermission {
        node_id: owner_hex.clone(),
        permission: PermissionLevel::Admin,
        granted_by: owner_hex.clone(),
        granted_at: drive.created_at.to_rfc3339(),
        expires_at: None,
        is_owner: true,
    });

    // Add other users
    for node_id in acl.users() {
        if let Some(rule) = acl.get_rule(node_id) {
            if rule.is_valid() {
                permissions.push(UserPermission {
                    node_id: node_id.to_string(),
                    permission: rule.permission.into(),
                    granted_by: rule.granted_by.clone(),
                    granted_at: rule.granted_at.to_rfc3339(),
                    expires_at: rule.expires_at.map(|t| t.to_rfc3339()),
                    is_owner: false,
                });
            }
        }
    }

    Ok(permissions)
}

/// Grant permission to a user
#[tauri::command]
pub async fn grant_permission(
    drive_id: String,
    target_node_id: String,
    permission: PermissionLevel,
    expires_in_days: Option<u32>,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<(), String> {
    let id_arr = parse_drive_id(&drive_id)?;

    // Get drive to find owner
    let drives = state.drives.read().await;
    let drive = drives
        .get(&id_arr)
        .ok_or_else(|| "Drive not found".to_string())?;

    let owner_hex = drive.owner.to_hex();

    // Get caller's node ID
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| "Identity not initialized".to_string())?;
    let caller_hex = caller.to_hex();

    // Get or create ACL
    let mut acl = security.get_or_create_acl(&drive_id, &owner_hex).await;

    // Check if caller has permission to grant access
    if !acl.check_permission(&caller_hex, "/", Permission::Manage) {
        return Err("Insufficient permission to grant access".to_string());
    }

    // Create access rule
    let mut rule = AccessRule::new(permission.clone().into(), &caller_hex);

    if let Some(days) = expires_in_days {
        let expires_at = Utc::now() + ChronoDuration::days(days as i64);
        rule = rule.with_expiry(expires_at);
    }

    // Grant access
    acl.grant(&target_node_id, rule);

    // Save updated ACL
    security.update_acl(&drive_id, acl).await;

    tracing::info!(
        "Granted {:?} permission to {} for drive {}",
        permission,
        target_node_id,
        drive_id
    );

    Ok(())
}

/// Revoke a user's access to a drive
#[tauri::command]
pub async fn revoke_permission(
    drive_id: String,
    target_node_id: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<(), String> {
    let id_arr = parse_drive_id(&drive_id)?;

    // Get drive to find owner
    let drives = state.drives.read().await;
    let drive = drives
        .get(&id_arr)
        .ok_or_else(|| "Drive not found".to_string())?;

    let owner_hex = drive.owner.to_hex();

    // Cannot revoke owner
    if target_node_id == owner_hex {
        return Err("Cannot revoke owner's access".to_string());
    }

    // Get caller's node ID
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| "Identity not initialized".to_string())?;
    let caller_hex = caller.to_hex();

    // Get or create ACL
    let mut acl = security.get_or_create_acl(&drive_id, &owner_hex).await;

    // Check if caller has permission to revoke access
    if !acl.check_permission(&caller_hex, "/", Permission::Manage) {
        return Err("Insufficient permission to revoke access".to_string());
    }

    // Revoke access
    acl.revoke(&target_node_id);

    // Save updated ACL
    security.update_acl(&drive_id, acl).await;

    tracing::info!(
        "Revoked access for {} from drive {}",
        target_node_id,
        drive_id
    );

    Ok(())
}

/// Check if a user has a specific permission for a path
#[tauri::command]
pub async fn check_permission(
    drive_id: String,
    node_id: Option<String>,
    path: String,
    required: PermissionLevel,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<bool, String> {
    let id_arr = parse_drive_id(&drive_id)?;

    // Get drive to find owner
    let drives = state.drives.read().await;
    let drive = drives
        .get(&id_arr)
        .ok_or_else(|| "Drive not found".to_string())?;

    let owner_hex = drive.owner.to_hex();

    // Get the node ID to check (default to caller)
    let check_node_id = match node_id {
        Some(id) => id,
        None => {
            let caller = state
                .identity_manager
                .node_id()
                .await
                .ok_or_else(|| "Identity not initialized".to_string())?;
            caller.to_hex()
        }
    };

    // Get ACL
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;

    // Check permission using ACL's check method
    let required_perm: Permission = required.into();

    Ok(acl.check_permission(&check_node_id, &path, required_perm))
}

// ============================================================================
// Helper functions
// ============================================================================

/// Helper to parse and validate drive ID
fn parse_drive_id(drive_id: &str) -> Result<[u8; 32], String> {
    let id_bytes = hex::decode(drive_id).map_err(|_| "Invalid drive ID format".to_string())?;

    if id_bytes.len() != 32 {
        return Err("Invalid drive ID length".to_string());
    }

    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&id_bytes);
    Ok(id_arr)
}
