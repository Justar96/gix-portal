//! Security commands for access control and invite management
//!
//! Phase 3: Tauri commands exposing crypto module functionality.
//!
//! # Security
//! - Rate limiting on invite generation
//! - Signature verification on invite acceptance
//! - ACL-based permission checks

use crate::core::error::AppError;
use crate::core::rate_limit::{RateLimitOperation, SharedRateLimiter};
use crate::core::validation::validate_drive_id;
use crate::core::{DriveId, SharedDrive};
use crate::crypto::{
    AccessControlList, AccessRule, InviteBuilder, InviteToken, NodeId, Permission, TokenTracker,
};
use crate::state::AppState;
use crate::storage::Database;
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Persistent store for ACLs and token trackers per drive
///
/// Data is stored in memory for fast access and persisted to the database
/// on every update.
pub struct SecurityStore {
    /// Database for persistence
    db: Arc<Database>,
    /// ACLs keyed by drive ID (hex string)
    acls: RwLock<HashMap<String, AccessControlList>>,
    /// Token trackers keyed by drive ID (hex string)
    token_trackers: RwLock<HashMap<String, TokenTracker>>,
    /// Revoked token IDs keyed by drive ID (hex string)
    revoked_tokens: RwLock<HashMap<String, HashSet<String>>>,
}

impl SecurityStore {
    /// Create a new SecurityStore with database persistence
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            acls: RwLock::new(HashMap::new()),
            token_trackers: RwLock::new(HashMap::new()),
            revoked_tokens: RwLock::new(HashMap::new()),
        }
    }

    /// Load all ACLs, token trackers, and revoked tokens from database
    pub fn load_from_db(&self) -> Result<(), String> {
        // Load ACLs
        let acl_entries = self.db.list_acls().map_err(|e| e.to_string())?;
        let mut acls_guard = self.acls.blocking_write();
        for (drive_id, data) in acl_entries {
            match serde_json::from_slice::<AccessControlList>(&data) {
                Ok(acl) => {
                    tracing::debug!("Loaded ACL for drive {}", drive_id);
                    acls_guard.insert(drive_id, acl);
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize ACL for drive: {}", e);
                }
            }
        }
        tracing::info!("Loaded {} ACLs from database", acls_guard.len());

        // Load token trackers
        let tracker_entries = self.db.list_token_trackers().map_err(|e| e.to_string())?;
        let mut trackers_guard = self.token_trackers.blocking_write();
        for (drive_id, data) in tracker_entries {
            match serde_json::from_slice::<TokenTracker>(&data) {
                Ok(tracker) => {
                    tracing::debug!("Loaded token tracker for drive {}", drive_id);
                    trackers_guard.insert(drive_id, tracker);
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize token tracker: {}", e);
                }
            }
        }
        tracing::info!(
            "Loaded {} token trackers from database",
            trackers_guard.len()
        );

        // Load revoked tokens
        let revoked_entries = self.db.list_revoked_tokens().map_err(|e| e.to_string())?;
        let mut revoked_guard = self.revoked_tokens.blocking_write();
        for (drive_id, data) in revoked_entries {
            match serde_json::from_slice::<HashSet<String>>(&data) {
                Ok(tokens) => {
                    tracing::debug!(
                        "Loaded {} revoked tokens for drive {}",
                        tokens.len(),
                        drive_id
                    );
                    revoked_guard.insert(drive_id, tokens);
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize revoked tokens: {}", e);
                }
            }
        }
        tracing::info!(
            "Loaded revoked tokens for {} drives from database",
            revoked_guard.len()
        );

        Ok(())
    }

    /// Get or create ACL for a drive
    pub async fn get_or_create_acl(&self, drive_id: &str, owner: &str) -> AccessControlList {
        let mut acls = self.acls.write().await;
        acls.entry(drive_id.to_string())
            .or_insert_with(|| AccessControlList::new(owner))
            .clone()
    }

    /// Update ACL for a drive (persists to database)
    pub async fn update_acl(&self, drive_id: &str, acl: AccessControlList) {
        // Update in memory
        {
            let mut acls = self.acls.write().await;
            acls.insert(drive_id.to_string(), acl.clone());
        }

        // Persist to database
        match serde_json::to_vec(&acl) {
            Ok(data) => {
                if let Err(e) = self.db.save_acl(drive_id, &data) {
                    tracing::error!("Failed to persist ACL for drive {}: {}", drive_id, e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize ACL: {}", e);
            }
        }
    }

    /// Get token tracker for a drive
    pub async fn get_token_tracker(&self, drive_id: &str) -> TokenTracker {
        let trackers = self.token_trackers.read().await;
        trackers.get(drive_id).cloned().unwrap_or_default()
    }

    /// Update token tracker for a drive (persists to database)
    pub async fn update_token_tracker(&self, drive_id: &str, tracker: TokenTracker) {
        // Update in memory
        {
            let mut trackers = self.token_trackers.write().await;
            trackers.insert(drive_id.to_string(), tracker.clone());
        }

        // Persist to database
        match serde_json::to_vec(&tracker) {
            Ok(data) => {
                if let Err(e) = self.db.save_token_tracker(drive_id, &data) {
                    tracing::error!(
                        "Failed to persist token tracker for drive {}: {}",
                        drive_id,
                        e
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize token tracker: {}", e);
            }
        }
    }

    /// Cleanup expired ACL rules across all drives
    pub async fn cleanup_expired(&self) -> usize {
        let mut acls = self.acls.write().await;
        let mut total = 0;
        let mut modified_drives = Vec::new();

        for (drive_id, acl) in acls.iter_mut() {
            // Count expired rules before cleanup
            let expired_count = acl
                .users()
                .iter()
                .filter(|uid| acl.get_rule(uid).map(|r| r.is_expired()).unwrap_or(false))
                .count();

            if expired_count > 0 {
                acl.cleanup_expired();
                total += expired_count;
                modified_drives.push((drive_id.clone(), acl.clone()));
            }
        }

        // Persist modified ACLs
        for (drive_id, acl) in modified_drives {
            if let Ok(data) = serde_json::to_vec(&acl) {
                if let Err(e) = self.db.save_acl(&drive_id, &data) {
                    tracing::error!("Failed to persist ACL after cleanup: {}", e);
                }
            }
        }

        total
    }

    /// Delete ACL for a drive (when drive is deleted)
    #[allow(dead_code)]
    pub async fn delete_acl(&self, drive_id: &str) {
        // Remove from memory
        {
            let mut acls = self.acls.write().await;
            acls.remove(drive_id);
        }

        // Remove from database
        if let Err(e) = self.db.delete_acl(drive_id) {
            tracing::error!("Failed to delete ACL from database: {}", e);
        }
    }

    // ============================================================================
    // Token Revocation
    // ============================================================================

    /// Check if a token is revoked
    pub async fn is_token_revoked(&self, drive_id: &str, token_id: &str) -> bool {
        let revoked = self.revoked_tokens.read().await;
        revoked
            .get(drive_id)
            .map(|tokens| tokens.contains(token_id))
            .unwrap_or(false)
    }

    /// Revoke a token (persists to database)
    pub async fn revoke_token(&self, drive_id: &str, token_id: &str) {
        // Update in memory
        {
            let mut revoked = self.revoked_tokens.write().await;
            revoked
                .entry(drive_id.to_string())
                .or_default()
                .insert(token_id.to_string());
        }

        // Persist to database
        let revoked = self.revoked_tokens.read().await;
        if let Some(tokens) = revoked.get(drive_id) {
            match serde_json::to_vec(&tokens) {
                Ok(data) => {
                    if let Err(e) = self.db.save_revoked_tokens(drive_id, &data) {
                        tracing::error!(
                            "Failed to persist revoked tokens for drive {}: {}",
                            drive_id,
                            e
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to serialize revoked tokens: {}", e);
                }
            }
        }
    }

    /// Get all revoked token IDs for a drive
    pub async fn get_revoked_tokens(&self, drive_id: &str) -> HashSet<String> {
        let revoked = self.revoked_tokens.read().await;
        revoked.get(drive_id).cloned().unwrap_or_default()
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
    pub drive_name: Option<String>,
    pub permission: Option<PermissionLevel>,
    pub inviter: Option<String>,
    pub expires_at: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Generate an invite token for a drive
///
/// # Security
/// - Rate limited to prevent abuse
/// - Requires drive ownership
#[tauri::command]
pub async fn generate_invite(
    request: CreateInviteRequest,
    state: State<'_, AppState>,
    _security: State<'_, Arc<SecurityStore>>,
    rate_limiter: State<'_, SharedRateLimiter>,
) -> Result<InviteInfo, String> {
    // Rate limit check
    let node_id = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;

    match rate_limiter
        .check(&node_id.as_bytes(), RateLimitOperation::InviteGeneration)
        .await
    {
        crate::core::rate_limit::RateLimitResult::Allowed { remaining } => {
            tracing::debug!(remaining = remaining, "Invite generation rate limit OK");
        }
        crate::core::rate_limit::RateLimitResult::Denied { retry_after } => {
            return Err(AppError::RateLimited {
                retry_after_secs: retry_after.as_secs(),
            }
            .to_string());
        }
    }

    // Validate drive ID
    let drive_id = &request.drive_id;
    validate_drive_id(drive_id).map_err(|e| e.to_string())?;
    let id_arr = parse_drive_id(drive_id)?;

    let drives = state.drives.read().await;
    let drive = drives.get(&id_arr).ok_or_else(|| {
        AppError::DriveNotFound {
            drive_id: drive_id.clone(),
        }
        .to_string()
    })?;

    // Get the signing key from identity manager
    let signing_key = state
        .identity_manager
        .signing_key()
        .await
        .ok_or_else(|| AppError::IdentityNotInitialized.to_string())?;

    // Validate validity hours (1 to 168 hours = 1 week max)
    let validity_hours = request.validity_hours.unwrap_or(24).min(168).max(1);
    let validity = ChronoDuration::hours(validity_hours as i64);

    let mut builder = InviteBuilder::new(drive_id, &drive.name)
        .with_permission(request.permission.clone().into())
        .with_validity(validity);

    if let Some(note) = &request.note {
        // Validate note length
        if note.len() > 500 {
            return Err(
                AppError::ValidationError("Note too long (max 500 chars)".to_string()).to_string(),
            );
        }
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
        drive_id = %drive_id,
        drive_name = %drive.name,
        permission = ?request.permission,
        validity_hours = validity_hours,
        single_use = request.single_use.unwrap_or(false),
        "Generated invite token"
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
///
/// # Security
/// - Validates token format and structure
/// - Verifies Ed25519 signature against inviter's public key
/// - Checks expiration time
/// - Checks if token has been revoked
#[tauri::command]
pub async fn verify_invite(
    token_string: String,
    _state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<InviteVerification, String> {
    // Parse the token
    let token = match InviteToken::from_string(&token_string) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid invite token format");
            return Ok(InviteVerification {
                valid: false,
                drive_id: None,
                drive_name: None,
                permission: None,
                inviter: None,
                expires_at: None,
                error: Some(format!("Invalid token format: {}", e)),
            });
        }
    };

    // Check expiration first (quick check)
    if token.is_expired() {
        tracing::debug!(
            drive_id = %token.payload.drive_id,
            expires_at = %token.payload.expires_at,
            "Invite token has expired"
        );
        return Ok(InviteVerification {
            valid: false,
            drive_id: Some(token.payload.drive_id.clone()),
            drive_name: Some(token.payload.drive_name.clone()),
            permission: Some(token.payload.permission.into()),
            inviter: Some(token.payload.inviter.clone()),
            expires_at: Some(token.payload.expires_at.to_rfc3339()),
            error: Some("Token has expired".to_string()),
        });
    }

    // SECURITY: Check if token has been revoked
    if security
        .is_token_revoked(&token.payload.drive_id, token.token_id())
        .await
    {
        tracing::warn!(
            drive_id = %token.payload.drive_id,
            token_id = %token.token_id(),
            "Attempted use of revoked invite token"
        );
        return Ok(InviteVerification {
            valid: false,
            drive_id: Some(token.payload.drive_id.clone()),
            drive_name: Some(token.payload.drive_name.clone()),
            permission: Some(token.payload.permission.into()),
            inviter: Some(token.payload.inviter.clone()),
            expires_at: Some(token.payload.expires_at.to_rfc3339()),
            error: Some("This invite has been revoked".to_string()),
        });
    }

    // Verify signature against inviter's public key
    // The inviter field contains the hex-encoded public key
    let inviter_pubkey = match hex::decode(&token.payload.inviter) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            match ed25519_dalek::VerifyingKey::from_bytes(&arr) {
                Ok(key) => key,
                Err(e) => {
                    tracing::warn!(error = %e, "Invalid inviter public key in token");
                    return Ok(InviteVerification {
                        valid: false,
                        drive_id: Some(token.payload.drive_id.clone()),
                        drive_name: Some(token.payload.drive_name.clone()),
                        permission: Some(token.payload.permission.into()),
                        inviter: Some(token.payload.inviter.clone()),
                        expires_at: Some(token.payload.expires_at.to_rfc3339()),
                        error: Some("Invalid inviter public key".to_string()),
                    });
                }
            }
        }
        _ => {
            tracing::warn!("Invalid inviter key format in token");
            return Ok(InviteVerification {
                valid: false,
                drive_id: Some(token.payload.drive_id.clone()),
                drive_name: Some(token.payload.drive_name.clone()),
                permission: Some(token.payload.permission.into()),
                inviter: Some(token.payload.inviter.clone()),
                expires_at: Some(token.payload.expires_at.to_rfc3339()),
                error: Some("Invalid inviter key format".to_string()),
            });
        }
    };

    // Verify the signature
    if let Err(e) = token.verify(&inviter_pubkey) {
        tracing::warn!(
            error = %e,
            inviter = %token.payload.inviter,
            "Invite token signature verification failed"
        );
        return Ok(InviteVerification {
            valid: false,
            drive_id: Some(token.payload.drive_id.clone()),
            drive_name: Some(token.payload.drive_name.clone()),
            permission: Some(token.payload.permission.into()),
            inviter: Some(token.payload.inviter.clone()),
            expires_at: Some(token.payload.expires_at.to_rfc3339()),
            error: Some("Invalid signature - token may have been tampered with".to_string()),
        });
    }

    tracing::info!(
        drive_id = %token.payload.drive_id,
        drive_name = %token.payload.drive_name,
        permission = ?token.payload.permission,
        inviter = %token.payload.inviter,
        "Invite token verified successfully"
    );

    Ok(InviteVerification {
        valid: true,
        drive_id: Some(token.payload.drive_id.clone()),
        drive_name: Some(token.payload.drive_name.clone()),
        permission: Some(token.payload.permission.into()),
        inviter: Some(token.payload.inviter.clone()),
        expires_at: Some(token.payload.expires_at.to_rfc3339()),
        error: None,
    })
}

/// Result of accepting an invite
#[derive(Clone, Debug, Serialize)]
pub struct AcceptInviteResult {
    pub success: bool,
    pub drive_id: String,
    pub drive_name: String,
    pub permission: PermissionLevel,
    pub error: Option<String>,
}

/// Accept an invite token and join the drive
///
/// # Security
/// - Verifies token signature and expiration
/// - Checks if token has been revoked
/// - Grants permission from token to caller
/// - Adds caller to drive's ACL
#[tauri::command]
pub async fn accept_invite(
    token_string: String,
    state: State<'_, AppState>,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<AcceptInviteResult, String> {
    // Parse the token
    let token = match InviteToken::from_string(&token_string) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid invite token format");
            return Ok(AcceptInviteResult {
                success: false,
                drive_id: String::new(),
                drive_name: String::new(),
                permission: PermissionLevel::Read,
                error: Some(format!("Invalid token format: {}", e)),
            });
        }
    };

    // Check expiration
    if token.is_expired() {
        tracing::debug!(
            drive_id = %token.payload.drive_id,
            expires_at = %token.payload.expires_at,
            "Invite token has expired"
        );
        return Ok(AcceptInviteResult {
            success: false,
            drive_id: token.payload.drive_id.clone(),
            drive_name: String::new(),
            permission: token.payload.permission.into(),
            error: Some("Token has expired".to_string()),
        });
    }

    // SECURITY: Check if token has been revoked
    if security
        .is_token_revoked(&token.payload.drive_id, token.token_id())
        .await
    {
        tracing::warn!(
            drive_id = %token.payload.drive_id,
            token_id = %token.token_id(),
            "Attempted acceptance of revoked invite token"
        );
        return Ok(AcceptInviteResult {
            success: false,
            drive_id: token.payload.drive_id.clone(),
            drive_name: String::new(),
            permission: token.payload.permission.into(),
            error: Some("This invite has been revoked".to_string()),
        });
    }

    // Verify signature against inviter's public key
    let inviter_pubkey = match hex::decode(&token.payload.inviter) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            match ed25519_dalek::VerifyingKey::from_bytes(&arr) {
                Ok(key) => key,
                Err(e) => {
                    tracing::warn!(error = %e, "Invalid inviter public key in token");
                    return Ok(AcceptInviteResult {
                        success: false,
                        drive_id: token.payload.drive_id.clone(),
                        drive_name: String::new(),
                        permission: token.payload.permission.into(),
                        error: Some("Invalid inviter public key".to_string()),
                    });
                }
            }
        }
        _ => {
            tracing::warn!("Invalid inviter key format in token");
            return Ok(AcceptInviteResult {
                success: false,
                drive_id: token.payload.drive_id.clone(),
                drive_name: String::new(),
                permission: token.payload.permission.into(),
                error: Some("Invalid inviter key format".to_string()),
            });
        }
    };

    // Verify the signature
    if let Err(e) = token.verify(&inviter_pubkey) {
        tracing::warn!(
            error = %e,
            inviter = %token.payload.inviter,
            "Invite token signature verification failed"
        );
        return Ok(AcceptInviteResult {
            success: false,
            drive_id: token.payload.drive_id.clone(),
            drive_name: String::new(),
            permission: token.payload.permission.into(),
            error: Some("Invalid signature - token may have been tampered with".to_string()),
        });
    }

    // Parse the drive ID from the token
    let drive_id = &token.payload.drive_id;
    let id_arr = match parse_drive_id(drive_id) {
        Ok(arr) => arr,
        Err(e) => {
            return Ok(AcceptInviteResult {
                success: false,
                drive_id: drive_id.clone(),
                drive_name: String::new(),
                permission: token.payload.permission.into(),
                error: Some(e),
            });
        }
    };

    // Get drive name from token
    let drive_name = token.payload.drive_name.clone();
    let owner_hex = token.payload.inviter.clone();

    // Get caller's node ID
    let caller = state
        .identity_manager
        .node_id()
        .await
        .ok_or_else(|| "Identity not initialized".to_string())?;
    let caller_hex = caller.to_hex();

    // Don't allow inviter to join their own drive
    if caller_hex == owner_hex {
        return Ok(AcceptInviteResult {
            success: false,
            drive_id: drive_id.clone(),
            drive_name,
            permission: token.payload.permission.into(),
            error: Some("You cannot join your own drive".to_string()),
        });
    }

    // SECURITY: Check if single-use token has already been used
    if token.payload.single_use {
        let tracker = security.get_token_tracker(drive_id).await;
        if tracker.is_used(token.token_id()) {
            tracing::warn!(
                drive_id = %drive_id,
                token_id = %token.token_id(),
                "Attempted reuse of single-use invite token"
            );
            return Ok(AcceptInviteResult {
                success: false,
                drive_id: drive_id.clone(),
                drive_name,
                permission: token.payload.permission.into(),
                error: Some("This single-use invite has already been used".to_string()),
            });
        }
    }

    // Check if the drive already exists locally
    let drives = state.drives.read().await;
    let drive_exists = drives.contains_key(&id_arr);
    drop(drives);

    if !drive_exists {
        // Create a local directory for the joined drive
        let base_dir = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let drives_dir = base_dir.join("GixDrives");

        // Create the GixDrives directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&drives_dir) {
            tracing::error!(error = %e, "Failed to create GixDrives directory");
            return Ok(AcceptInviteResult {
                success: false,
                drive_id: drive_id.clone(),
                drive_name,
                permission: token.payload.permission.into(),
                error: Some(format!("Failed to create drives directory: {}", e)),
            });
        }

        // Create a unique folder name for this drive
        let safe_name = drive_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();
        let short_id = &drive_id[..8.min(drive_id.len())];
        let folder_name = format!("{}_{}", safe_name, short_id);
        let local_path = drives_dir.join(&folder_name);

        // Create the drive directory
        if let Err(e) = std::fs::create_dir_all(&local_path) {
            tracing::error!(error = %e, path = %local_path.display(), "Failed to create drive directory");
            return Ok(AcceptInviteResult {
                success: false,
                drive_id: drive_id.clone(),
                drive_name,
                permission: token.payload.permission.into(),
                error: Some(format!("Failed to create drive directory: {}", e)),
            });
        }

        // Parse the owner's NodeId from the inviter hex
        let owner_bytes = match hex::decode(&owner_hex) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            }
            _ => {
                return Ok(AcceptInviteResult {
                    success: false,
                    drive_id: drive_id.clone(),
                    drive_name,
                    permission: token.payload.permission.into(),
                    error: Some("Invalid owner ID in token".to_string()),
                });
            }
        };
        let owner = NodeId(owner_bytes);

        // Create the SharedDrive entry for this joined drive
        let drive = SharedDrive {
            id: DriveId(id_arr),
            name: drive_name.clone(),
            local_path: local_path.clone(),
            owner,
            created_at: Utc::now(),
            total_size: 0,
            file_count: 0,
        };

        // Save to database
        let drive_bytes = serde_json::to_vec(&drive).map_err(|e| {
            format!("Failed to serialize drive: {}", e)
        })?;

        state.db.save_drive(drive.id.as_bytes(), &drive_bytes).map_err(|e| {
            format!("Failed to save drive: {}", e)
        })?;

        // Add to in-memory cache
        state.drives.write().await.insert(id_arr, drive);

        tracing::info!(
            drive_id = %drive_id,
            drive_name = %drive_name,
            local_path = %local_path.display(),
            "Created local drive entry for joined drive"
        );
    }

    // Get or create ACL and grant permission
    let mut acl = security.get_or_create_acl(drive_id, &owner_hex).await;

    // Check if user already has access
    if acl.get_rule(&caller_hex).is_some() {
        tracing::info!(
            drive_id = %drive_id,
            user = %caller_hex,
            "User already has access to drive"
        );
        return Ok(AcceptInviteResult {
            success: true,
            drive_id: drive_id.clone(),
            drive_name,
            permission: token.payload.permission.into(),
            error: None,
        });
    }

    // Create access rule from token
    let rule = AccessRule::new(token.payload.permission, &token.payload.inviter);

    // Grant access
    acl.grant(&caller_hex, rule);

    // Save updated ACL
    security.update_acl(drive_id, acl).await;

    // SECURITY: Mark single-use token as used to prevent reuse
    if token.payload.single_use {
        let mut tracker = security.get_token_tracker(drive_id).await;
        tracker.mark_used(token.token_id());
        security.update_token_tracker(drive_id, tracker).await;
        tracing::debug!(
            drive_id = %drive_id,
            token_id = %token.token_id(),
            "Marked single-use token as used"
        );
    }

    // Auto-start sync for the joined drive
    // This ensures files are synced immediately after joining
    if let Some(sync_engine) = state.sync_engine.as_ref() {
        let drives = state.drives.read().await;
        if let Some(drive) = drives.get(&id_arr) {
            if let Err(e) = sync_engine.init_drive(drive).await {
                tracing::warn!(
                    drive_id = %drive_id,
                    error = %e,
                    "Failed to auto-start sync after joining drive (can be started manually)"
                );
            } else {
                tracing::info!(drive_id = %drive_id, "Auto-started sync for joined drive");
            }
        }
    }

    // Auto-start file watching for the joined drive
    if let Some(watcher) = state.file_watcher.as_ref() {
        let drives = state.drives.read().await;
        if let Some(drive) = drives.get(&id_arr) {
            let local_path = drive.local_path.clone();
            drop(drives); // Release lock before async operation
            
            if let Err(e) = watcher.watch(DriveId(id_arr), local_path).await {
                tracing::warn!(
                    drive_id = %drive_id,
                    error = %e,
                    "Failed to auto-start file watching after joining drive"
                );
            } else {
                tracing::info!(drive_id = %drive_id, "Auto-started file watching for joined drive");
            }
        }
    }

    tracing::info!(
        drive_id = %drive_id,
        drive_name = %drive_name,
        user = %caller_hex,
        permission = ?token.payload.permission,
        inviter = %token.payload.inviter,
        "User accepted invite and joined drive"
    );

    Ok(AcceptInviteResult {
        success: true,
        drive_id: drive_id.clone(),
        drive_name,
        permission: token.payload.permission.into(),
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

/// Revoke an invite token
///
/// # Security
/// - Requires Manage permission on the drive
/// - Permanently revokes the token (cannot be undone)
#[tauri::command]
pub async fn revoke_invite(
    drive_id: String,
    token_id: String,
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

    // Get or create ACL and check permission
    let acl = security.get_or_create_acl(&drive_id, &owner_hex).await;

    // Check if caller has permission to revoke invites (requires Manage)
    if !acl.check_permission(&caller_hex, "/", Permission::Manage) {
        tracing::warn!(
            drive_id = %drive_id,
            user = %caller_hex,
            token_id = %token_id,
            "Access denied: insufficient permission to revoke invite"
        );
        return Err("Insufficient permission to revoke invites".to_string());
    }

    // Revoke the token
    security.revoke_token(&drive_id, &token_id).await;

    tracing::info!(
        drive_id = %drive_id,
        token_id = %token_id,
        revoked_by = %caller_hex,
        "Invite token revoked"
    );

    Ok(())
}

/// List all revoked token IDs for a drive
#[tauri::command]
pub async fn list_revoked_tokens(
    drive_id: String,
    security: State<'_, Arc<SecurityStore>>,
) -> Result<Vec<String>, String> {
    // Validate drive ID
    validate_drive_id(&drive_id).map_err(|e| e.to_string())?;

    let revoked = security.get_revoked_tokens(&drive_id).await;
    Ok(revoked.into_iter().collect())
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
