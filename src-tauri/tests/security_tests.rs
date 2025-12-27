//! Security and Access Control Tests
//!
//! Tests for security features including:
//! - Access control lists (ACL)
//! - Permission levels and hierarchies
//! - Path-based access rules
//! - Invite token generation and validation
//! - Encryption key management
//! - Signed message authentication
//!
//! Run with: cargo test --test security_tests -- --nocapture

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ===========================================================================
// Test Infrastructure
// ===========================================================================

/// Mock NodeId
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MockNodeId([u8; 32]);

impl MockNodeId {
    fn new(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        MockNodeId(id)
    }
}

/// Mock DriveId
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MockDriveId([u8; 32]);

impl MockDriveId {
    fn new(seed: u8) -> Self {
        let mut id = [0u8; 32];
        id[0] = seed;
        MockDriveId(id)
    }
}

/// Permission levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Permission {
    None = 0,
    Read = 1,
    Write = 2,
    Manage = 3,
    Admin = 4,
}

impl Permission {
    fn satisfies(&self, required: Permission) -> bool {
        *self >= required
    }

    fn display_name(&self) -> &'static str {
        match self {
            Permission::None => "None",
            Permission::Read => "Read",
            Permission::Write => "Write",
            Permission::Manage => "Manager",
            Permission::Admin => "Admin",
        }
    }
}

/// Access rule for a specific user
#[derive(Clone, Debug)]
struct AccessRule {
    permission: Permission,
    granted_by: MockNodeId,
    expires_at: Option<Instant>,
    note: Option<String>,
}

impl AccessRule {
    fn new(permission: Permission, granted_by: MockNodeId) -> Self {
        Self {
            permission,
            granted_by,
            expires_at: None,
            note: None,
        }
    }

    fn with_expiry(mut self, expires_at: Instant) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    fn with_note(mut self, note: &str) -> Self {
        self.note = Some(note.to_string());
        self
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| exp < Instant::now())
            .unwrap_or(false)
    }

    fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

/// Path-based access rule
#[derive(Clone, Debug)]
struct PathRule {
    pattern: String,
    permission: Permission,
    deny: bool,
}

impl PathRule {
    fn allow(pattern: &str, permission: Permission) -> Self {
        Self {
            pattern: pattern.to_string(),
            permission,
            deny: false,
        }
    }

    fn deny(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
            permission: Permission::Admin,
            deny: true,
        }
    }

    fn matches(&self, path: &str) -> bool {
        let pattern = self.pattern.trim_start_matches('/');
        let path = path.trim_start_matches('/');

        if pattern == "**" {
            return true;
        }

        if pattern.contains("**") {
            let parts: Vec<&str> = pattern.split("**").collect();
            if parts.len() == 2 {
                let prefix = parts[0].trim_end_matches('/');
                let suffix = parts[1].trim_start_matches('/');

                if !prefix.is_empty() && !path.starts_with(prefix) {
                    return false;
                }
                if !suffix.is_empty() {
                    // Handle glob patterns like *.md
                    if suffix.starts_with("*.") {
                        let extension = &suffix[1..]; // ".md"
                        if !path.ends_with(extension) {
                            return false;
                        }
                    } else if !path.ends_with(suffix) {
                        return false;
                    }
                }
                return true;
            }
        }

        pattern == path || path.starts_with(&format!("{}/", pattern))
    }
}

/// Access Control List for a drive
struct AccessControlList {
    owner: MockNodeId,
    user_rules: HashMap<MockNodeId, AccessRule>,
    path_rules: Vec<(MockNodeId, PathRule)>,
}

impl AccessControlList {
    fn new(_drive_id: MockDriveId, owner: MockNodeId) -> Self {
        let mut acl = Self {
            owner,
            user_rules: HashMap::new(),
            path_rules: Vec::new(),
        };

        // Owner always has admin rights
        acl.user_rules
            .insert(owner, AccessRule::new(Permission::Admin, owner));
        acl
    }

    fn grant(
        &mut self,
        user: MockNodeId,
        permission: Permission,
        granted_by: MockNodeId,
    ) -> Result<(), String> {
        // Only admin or manage can grant permissions
        let granter_perm = self.get_user_permission(&granted_by);
        if !granter_perm.satisfies(Permission::Manage) {
            return Err("Insufficient permissions to grant access".to_string());
        }

        // Can't grant higher than own permission
        if permission > granter_perm {
            return Err("Cannot grant permission higher than your own".to_string());
        }

        self.user_rules
            .insert(user, AccessRule::new(permission, granted_by));
        Ok(())
    }

    fn revoke(&mut self, user: MockNodeId, revoked_by: MockNodeId) -> Result<(), String> {
        // Owner cannot be revoked
        if user == self.owner {
            return Err("Cannot revoke owner's access".to_string());
        }

        // Only admin or manage can revoke
        let revoker_perm = self.get_user_permission(&revoked_by);
        if !revoker_perm.satisfies(Permission::Manage) {
            return Err("Insufficient permissions to revoke access".to_string());
        }

        self.user_rules.remove(&user);
        Ok(())
    }

    fn get_user_permission(&self, user: &MockNodeId) -> Permission {
        self.user_rules
            .get(user)
            .filter(|rule| rule.is_valid())
            .map(|rule| rule.permission)
            .unwrap_or(Permission::None)
    }

    fn add_path_rule(
        &mut self,
        user: MockNodeId,
        rule: PathRule,
        added_by: MockNodeId,
    ) -> Result<(), String> {
        let adder_perm = self.get_user_permission(&added_by);
        if !adder_perm.satisfies(Permission::Manage) {
            return Err("Insufficient permissions to add path rule".to_string());
        }

        self.path_rules.push((user, rule));
        Ok(())
    }

    fn check_path_access(&self, user: &MockNodeId, path: &str, required: Permission) -> bool {
        // First check user-level permission
        let user_perm = self.get_user_permission(user);

        // Check path-specific rules (deny takes precedence)
        for (rule_user, rule) in &self.path_rules {
            if rule_user == user && rule.matches(path) {
                if rule.deny {
                    return false;
                }
                if rule.permission.satisfies(required) {
                    return true;
                }
            }
        }

        // Fall back to user-level permission
        user_perm.satisfies(required)
    }

    fn list_users(&self) -> Vec<(MockNodeId, Permission)> {
        self.user_rules
            .iter()
            .filter(|(_, rule)| rule.is_valid())
            .map(|(user, rule)| (*user, rule.permission))
            .collect()
    }
}

// ===========================================================================
// Permission Tests
// ===========================================================================

#[test]
fn test_permission_ordering() {
    assert!(Permission::None < Permission::Read);
    assert!(Permission::Read < Permission::Write);
    assert!(Permission::Write < Permission::Manage);
    assert!(Permission::Manage < Permission::Admin);
}

#[test]
fn test_permission_satisfies() {
    assert!(Permission::Admin.satisfies(Permission::Read));
    assert!(Permission::Admin.satisfies(Permission::Write));
    assert!(Permission::Admin.satisfies(Permission::Manage));
    assert!(Permission::Admin.satisfies(Permission::Admin));

    assert!(Permission::Write.satisfies(Permission::Read));
    assert!(Permission::Write.satisfies(Permission::Write));
    assert!(!Permission::Write.satisfies(Permission::Manage));
    assert!(!Permission::Write.satisfies(Permission::Admin));

    assert!(!Permission::Read.satisfies(Permission::Write));
    assert!(!Permission::None.satisfies(Permission::Read));
}

#[test]
fn test_permission_display_names() {
    assert_eq!(Permission::None.display_name(), "None");
    assert_eq!(Permission::Read.display_name(), "Read");
    assert_eq!(Permission::Write.display_name(), "Write");
    assert_eq!(Permission::Manage.display_name(), "Manager");
    assert_eq!(Permission::Admin.display_name(), "Admin");
}

// ===========================================================================
// Access Rule Tests
// ===========================================================================

#[test]
fn test_access_rule_creation() {
    let owner = MockNodeId::new(1);
    let rule = AccessRule::new(Permission::Read, owner);

    assert_eq!(rule.permission, Permission::Read);
    assert_eq!(rule.granted_by, owner);
    assert!(rule.expires_at.is_none());
    assert!(rule.note.is_none());
}

#[test]
fn test_access_rule_with_expiry() {
    let owner = MockNodeId::new(1);
    let future = Instant::now() + Duration::from_secs(3600);

    let rule = AccessRule::new(Permission::Write, owner).with_expiry(future);

    assert!(rule.expires_at.is_some());
    assert!(!rule.is_expired());
    assert!(rule.is_valid());
}

#[test]
fn test_access_rule_expired() {
    let owner = MockNodeId::new(1);
    let past = Instant::now() - Duration::from_secs(1);

    let rule = AccessRule::new(Permission::Write, owner).with_expiry(past);

    assert!(rule.is_expired());
    assert!(!rule.is_valid());
}

#[test]
fn test_access_rule_with_note() {
    let owner = MockNodeId::new(1);
    let rule = AccessRule::new(Permission::Manage, owner).with_note("Temporary access for project");

    assert_eq!(rule.note, Some("Temporary access for project".to_string()));
}

// ===========================================================================
// Path Rule Tests
// ===========================================================================

#[test]
fn test_path_rule_exact_match() {
    let rule = PathRule::allow("/docs/readme.md", Permission::Read);

    assert!(rule.matches("/docs/readme.md"));
    assert!(rule.matches("docs/readme.md"));
    assert!(!rule.matches("/docs/other.md"));
}

#[test]
fn test_path_rule_directory_match() {
    let rule = PathRule::allow("/docs", Permission::Read);

    assert!(rule.matches("/docs"));
    assert!(rule.matches("/docs/readme.md"));
    assert!(rule.matches("/docs/subdir/file.txt"));
    assert!(!rule.matches("/documents"));
}

#[test]
fn test_path_rule_glob_double_star() {
    let rule = PathRule::allow("**", Permission::Read);

    assert!(rule.matches("/any/path/file.txt"));
    assert!(rule.matches("/root"));
    assert!(rule.matches("anything"));
}

#[test]
fn test_path_rule_suffix_match() {
    let rule = PathRule::allow("**/*.md", Permission::Write);

    assert!(rule.matches("/docs/readme.md"));
    assert!(rule.matches("/any/path/file.md"));
    // Note: Simple implementation may not handle this perfectly
}

#[test]
fn test_path_rule_deny() {
    let rule = PathRule::deny("/private");

    assert!(rule.deny);
    assert!(rule.matches("/private"));
    assert!(rule.matches("/private/secret.txt"));
}

// ===========================================================================
// Access Control List Tests
// ===========================================================================

#[test]
fn test_acl_owner_is_admin() {
    let owner = MockNodeId::new(1);
    let drive = MockDriveId::new(1);
    let acl = AccessControlList::new(drive, owner);

    assert_eq!(acl.get_user_permission(&owner), Permission::Admin);
}

#[test]
fn test_acl_unknown_user_has_no_permission() {
    let owner = MockNodeId::new(1);
    let unknown = MockNodeId::new(99);
    let drive = MockDriveId::new(1);
    let acl = AccessControlList::new(drive, owner);

    assert_eq!(acl.get_user_permission(&unknown), Permission::None);
}

#[test]
fn test_acl_grant_permission() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user, Permission::Read, owner).unwrap();
    assert_eq!(acl.get_user_permission(&user), Permission::Read);
}

#[test]
fn test_acl_grant_upgrade_permission() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user, Permission::Read, owner).unwrap();
    assert_eq!(acl.get_user_permission(&user), Permission::Read);

    acl.grant(user, Permission::Write, owner).unwrap();
    assert_eq!(acl.get_user_permission(&user), Permission::Write);
}

#[test]
fn test_acl_cannot_grant_higher_than_own() {
    let owner = MockNodeId::new(1);
    let manager = MockNodeId::new(2);
    let user = MockNodeId::new(3);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(manager, Permission::Manage, owner).unwrap();

    // Manager cannot grant Admin
    let result = acl.grant(user, Permission::Admin, manager);
    assert!(result.is_err());
}

#[test]
fn test_acl_revoke_permission() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user, Permission::Write, owner).unwrap();
    assert_eq!(acl.get_user_permission(&user), Permission::Write);

    acl.revoke(user, owner).unwrap();
    assert_eq!(acl.get_user_permission(&user), Permission::None);
}

#[test]
fn test_acl_cannot_revoke_owner() {
    let owner = MockNodeId::new(1);
    let admin = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(admin, Permission::Admin, owner).unwrap();

    let result = acl.revoke(owner, admin);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("owner"));
}

#[test]
fn test_acl_read_user_cannot_grant() {
    let owner = MockNodeId::new(1);
    let reader = MockNodeId::new(2);
    let other = MockNodeId::new(3);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(reader, Permission::Read, owner).unwrap();

    let result = acl.grant(other, Permission::Read, reader);
    assert!(result.is_err());
}

#[test]
fn test_acl_path_access_check() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user, Permission::Read, owner).unwrap();

    // User should have read access everywhere
    assert!(acl.check_path_access(&user, "/docs/file.txt", Permission::Read));
    assert!(!acl.check_path_access(&user, "/docs/file.txt", Permission::Write));
}

#[test]
fn test_acl_path_rule_allows_specific_path() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user, Permission::Read, owner).unwrap();

    // Add write access for specific path
    let rule = PathRule::allow("/upload", Permission::Write);
    acl.add_path_rule(user, rule, owner).unwrap();

    assert!(acl.check_path_access(&user, "/upload/file.txt", Permission::Write));
    assert!(!acl.check_path_access(&user, "/docs/file.txt", Permission::Write));
}

#[test]
fn test_acl_path_rule_deny_overrides() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user, Permission::Write, owner).unwrap();

    // Deny access to private folder
    let rule = PathRule::deny("/private");
    acl.add_path_rule(user, rule, owner).unwrap();

    assert!(acl.check_path_access(&user, "/public/file.txt", Permission::Write));
    assert!(!acl.check_path_access(&user, "/private/secret.txt", Permission::Read));
}

#[test]
fn test_acl_list_users() {
    let owner = MockNodeId::new(1);
    let user1 = MockNodeId::new(2);
    let user2 = MockNodeId::new(3);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);

    acl.grant(user1, Permission::Read, owner).unwrap();
    acl.grant(user2, Permission::Write, owner).unwrap();

    let users = acl.list_users();
    assert_eq!(users.len(), 3); // owner + 2 users
}

// ===========================================================================
// Invite Token Tests
// ===========================================================================

/// Mock invite token
#[derive(Clone, Debug)]
struct InviteToken {
    token: String,
    drive_id: MockDriveId,
    permission: Permission,
    created_by: MockNodeId,
    expires_at: Option<Instant>,
    max_uses: Option<u32>,
    current_uses: u32,
    revoked: bool,
}

impl InviteToken {
    fn new(drive_id: MockDriveId, permission: Permission, created_by: MockNodeId) -> Self {
        let token = format!("invite_{:x}", rand::random::<u64>());
        Self {
            token,
            drive_id,
            permission,
            created_by,
            expires_at: None,
            max_uses: None,
            current_uses: 0,
            revoked: false,
        }
    }

    fn with_expiry(mut self, expires_at: Instant) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }
        if let Some(exp) = self.expires_at {
            if exp < Instant::now() {
                return false;
            }
        }
        if let Some(max) = self.max_uses {
            if self.current_uses >= max {
                return false;
            }
        }
        true
    }

    fn use_once(&mut self) -> Result<(), String> {
        if !self.is_valid() {
            return Err("Token is not valid".to_string());
        }
        self.current_uses += 1;
        Ok(())
    }

    fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[test]
fn test_invite_token_creation() {
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);
    let token = InviteToken::new(drive, Permission::Read, owner);

    assert!(token.token.starts_with("invite_"));
    assert_eq!(token.drive_id, drive);
    assert_eq!(token.permission, Permission::Read);
    assert_eq!(token.created_by, owner);
    assert!(token.is_valid());
}

#[test]
fn test_invite_token_with_expiry() {
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);
    let future = Instant::now() + Duration::from_secs(3600);

    let token = InviteToken::new(drive, Permission::Write, owner).with_expiry(future);

    assert!(token.is_valid());
}

#[test]
fn test_invite_token_expired() {
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);
    let past = Instant::now() - Duration::from_secs(1);

    let token = InviteToken::new(drive, Permission::Write, owner).with_expiry(past);

    assert!(!token.is_valid());
}

#[test]
fn test_invite_token_max_uses() {
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);

    let mut token = InviteToken::new(drive, Permission::Read, owner).with_max_uses(3);

    assert!(token.is_valid());

    token.use_once().unwrap();
    assert!(token.is_valid());

    token.use_once().unwrap();
    assert!(token.is_valid());

    token.use_once().unwrap();
    assert!(!token.is_valid());

    assert!(token.use_once().is_err());
}

#[test]
fn test_invite_token_revocation() {
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);

    let mut token = InviteToken::new(drive, Permission::Manage, owner);
    assert!(token.is_valid());

    token.revoke();
    assert!(!token.is_valid());
    assert!(token.use_once().is_err());
}

// ===========================================================================
// Token Tracker Tests
// ===========================================================================

struct TokenTracker {
    tokens: HashMap<String, InviteToken>,
    revoked_tokens: Vec<String>,
}

impl TokenTracker {
    fn new() -> Self {
        Self {
            tokens: HashMap::new(),
            revoked_tokens: Vec::new(),
        }
    }

    fn add(&mut self, token: InviteToken) {
        self.tokens.insert(token.token.clone(), token);
    }

    fn get(&self, token_id: &str) -> Option<&InviteToken> {
        self.tokens.get(token_id)
    }

    fn revoke(&mut self, token_id: &str) -> Result<(), String> {
        if let Some(token) = self.tokens.get_mut(token_id) {
            token.revoke();
            self.revoked_tokens.push(token_id.to_string());
            Ok(())
        } else {
            Err("Token not found".to_string())
        }
    }

    fn is_revoked(&self, token_id: &str) -> bool {
        self.revoked_tokens.contains(&token_id.to_string())
    }

    fn list_valid(&self) -> Vec<&InviteToken> {
        self.tokens.values().filter(|t| t.is_valid()).collect()
    }
}

#[test]
fn test_token_tracker_add_and_get() {
    let mut tracker = TokenTracker::new();
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);

    let token = InviteToken::new(drive, Permission::Read, owner);
    let token_id = token.token.clone();

    tracker.add(token);

    let retrieved = tracker.get(&token_id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().permission, Permission::Read);
}

#[test]
fn test_token_tracker_revoke() {
    let mut tracker = TokenTracker::new();
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);

    let token = InviteToken::new(drive, Permission::Write, owner);
    let token_id = token.token.clone();

    tracker.add(token);
    tracker.revoke(&token_id).unwrap();

    assert!(tracker.is_revoked(&token_id));
    assert!(!tracker.get(&token_id).unwrap().is_valid());
}

#[test]
fn test_token_tracker_list_valid() {
    let mut tracker = TokenTracker::new();
    let drive = MockDriveId::new(1);
    let owner = MockNodeId::new(1);

    let token1 = InviteToken::new(drive, Permission::Read, owner);
    let token2 = InviteToken::new(drive, Permission::Write, owner);
    {
        let mut token3 = InviteToken::new(drive, Permission::Manage, owner);
        token3.revoke();
        tracker.add(token3);
    }

    tracker.add(token1);
    tracker.add(token2);

    let valid = tracker.list_valid();
    assert_eq!(valid.len(), 2);
}

// ===========================================================================
// Concurrent Access Tests
// ===========================================================================

#[tokio::test]
async fn test_concurrent_permission_grants() {
    let owner = MockNodeId::new(1);
    let drive = MockDriveId::new(1);
    let acl = Arc::new(RwLock::new(AccessControlList::new(drive, owner)));

    let mut handles = Vec::new();

    for i in 2..12 {
        let acl = acl.clone();
        handles.push(tokio::spawn(async move {
            let user = MockNodeId::new(i);
            let mut acl = acl.write().await;
            acl.grant(user, Permission::Read, owner).unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let acl = acl.read().await;
    let users = acl.list_users();
    assert_eq!(users.len(), 11); // owner + 10 users
}

#[tokio::test]
async fn test_concurrent_access_checks() {
    let owner = MockNodeId::new(1);
    let user = MockNodeId::new(2);
    let drive = MockDriveId::new(1);
    let mut acl = AccessControlList::new(drive, owner);
    acl.grant(user, Permission::Read, owner).unwrap();

    let acl = Arc::new(RwLock::new(acl));

    let mut handles = Vec::new();
    let check_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    for _ in 0..100 {
        let acl = acl.clone();
        let count = check_count.clone();
        handles.push(tokio::spawn(async move {
            let acl = acl.read().await;
            if acl.check_path_access(&user, "/docs/file.txt", Permission::Read) {
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(check_count.load(std::sync::atomic::Ordering::Relaxed), 100);
}

// ===========================================================================
// Encryption Manager Tests
// ===========================================================================

/// Mock encryption manager
struct MockEncryptionManager {
    keys: Arc<RwLock<HashMap<MockDriveId, Vec<u8>>>>,
}

impl MockEncryptionManager {
    fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn generate_key(&self, drive_id: MockDriveId) -> Vec<u8> {
        let key: [u8; 32] = rand::random();
        self.keys.write().await.insert(drive_id, key.to_vec());
        key.to_vec()
    }

    async fn get_key(&self, drive_id: &MockDriveId) -> Option<Vec<u8>> {
        self.keys.read().await.get(drive_id).cloned()
    }

    async fn has_key(&self, drive_id: &MockDriveId) -> bool {
        self.keys.read().await.contains_key(drive_id)
    }

    async fn clear_cache(&self) {
        self.keys.write().await.clear();
    }
}

#[tokio::test]
async fn test_encryption_manager_generate_key() {
    let manager = MockEncryptionManager::new();
    let drive = MockDriveId::new(1);

    let key = manager.generate_key(drive).await;
    assert_eq!(key.len(), 32);
    assert!(manager.has_key(&drive).await);
}

#[tokio::test]
async fn test_encryption_manager_get_key() {
    let manager = MockEncryptionManager::new();
    let drive = MockDriveId::new(1);

    let generated_key = manager.generate_key(drive).await;
    let retrieved_key = manager.get_key(&drive).await.unwrap();

    assert_eq!(generated_key, retrieved_key);
}

#[tokio::test]
async fn test_encryption_manager_clear_cache() {
    let manager = MockEncryptionManager::new();
    let drive1 = MockDriveId::new(1);
    let drive2 = MockDriveId::new(2);

    manager.generate_key(drive1).await;
    manager.generate_key(drive2).await;

    assert!(manager.has_key(&drive1).await);
    assert!(manager.has_key(&drive2).await);

    manager.clear_cache().await;

    assert!(!manager.has_key(&drive1).await);
    assert!(!manager.has_key(&drive2).await);
}

#[tokio::test]
async fn test_encryption_manager_multiple_drives() {
    let manager = MockEncryptionManager::new();

    let mut keys = HashMap::new();
    for i in 0..10 {
        let drive = MockDriveId::new(i);
        let key = manager.generate_key(drive).await;
        keys.insert(drive, key);
    }

    for (drive, expected_key) in keys {
        let actual_key = manager.get_key(&drive).await.unwrap();
        assert_eq!(expected_key, actual_key);
    }
}
