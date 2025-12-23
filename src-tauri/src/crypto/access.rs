//! Access control for shared drives
//!
//! Provides permission management for drive operations.
//! Supports per-user and path-based permissions with optional expiration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Permission levels for drive access
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Can view files and download
    #[default]
    Read = 0,
    /// Can read + upload, modify, delete files
    Write = 1,
    /// Can write + manage other users (invite, change permissions)
    Manage = 2,
    /// Full control including key rotation, drive deletion
    Admin = 3,
}

impl Permission {
    /// Check if this permission grants at least the required level
    pub fn satisfies(&self, required: Permission) -> bool {
        *self >= required
    }

    /// Get the display name for this permission
    pub fn display_name(&self) -> &'static str {
        match self {
            Permission::Read => "Read",
            Permission::Write => "Read & Write",
            Permission::Manage => "Manager",
            Permission::Admin => "Admin",
        }
    }
}


/// An access rule for a specific user or path
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessRule {
    /// The permission level granted
    pub permission: Permission,
    /// When the access was granted
    pub granted_at: DateTime<Utc>,
    /// Who granted this access (NodeId hex)
    pub granted_by: String,
    /// Optional expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Optional note about the access
    pub note: Option<String>,
}

impl AccessRule {
    /// Create a new access rule
    pub fn new(permission: Permission, granted_by: &str) -> Self {
        Self {
            permission,
            granted_at: Utc::now(),
            granted_by: granted_by.to_string(),
            expires_at: None,
            note: None,
        }
    }

    /// Set expiration time
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set a note
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Check if the rule has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false)
    }

    /// Check if the rule is still valid
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

/// A path-based access rule for fine-grained control
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathRule {
    /// Path pattern (supports glob-like matching)
    pub pattern: String,
    /// The permission level for this path
    pub permission: Permission,
    /// Whether to deny instead of allow
    pub deny: bool,
}

impl PathRule {
    /// Create a new allow rule
    pub fn allow(pattern: impl Into<String>, permission: Permission) -> Self {
        Self {
            pattern: pattern.into(),
            permission,
            deny: false,
        }
    }

    /// Create a new deny rule
    pub fn deny(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            permission: Permission::Admin, // Level doesn't matter for deny
            deny: true,
        }
    }

    /// Check if the path matches this rule
    pub fn matches(&self, path: &str) -> bool {
        // Simple glob matching: * matches any characters within a segment, ** matches any path
        let pattern = self.pattern.trim_start_matches('/');
        let path = path.trim_start_matches('/');

        if pattern == "**" {
            return true;
        }

        if pattern.contains("**") {
            // Double star: match any path
            let parts: Vec<&str> = pattern.split("**").collect();
            if parts.len() == 2 {
                let prefix = parts[0].trim_end_matches('/');
                let suffix = parts[1].trim_start_matches('/');

                if !prefix.is_empty() && !path.starts_with(prefix) {
                    return false;
                }
                if !suffix.is_empty() && !path.ends_with(suffix) {
                    return false;
                }
                return true;
            }
        }

        // Single star: match characters within a path segment
        if pattern.contains('*') && !pattern.contains("**") {
            let pattern_parts: Vec<&str> = pattern.split('/').collect();
            let path_parts: Vec<&str> = path.split('/').collect();

            if pattern_parts.len() != path_parts.len() {
                return false;
            }

            for (pp, pathp) in pattern_parts.iter().zip(path_parts.iter()) {
                if !Self::segment_matches(pp, pathp) {
                    return false;
                }
            }
            return true;
        }

        // Exact match
        pattern == path
    }

    /// Match a single path segment with potential wildcards
    fn segment_matches(pattern: &str, segment: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if !pattern.contains('*') {
            return pattern == segment;
        }
        // Handle patterns like "*.txt" or "prefix*"
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            return segment.starts_with(prefix)
                && segment.ends_with(suffix)
                && segment.len() >= prefix.len() + suffix.len();
        }
        // Fall back to exact match for complex patterns
        pattern == segment
    }
}

/// Access Control List for a shared drive
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccessControlList {
    /// The drive owner's NodeId (hex) - always has Admin access
    owner: String,
    /// Per-user access rules (NodeId hex -> AccessRule)
    user_rules: HashMap<String, AccessRule>,
    /// Path-based rules (evaluated in order)
    path_rules: Vec<PathRule>,
}

impl AccessControlList {
    /// Create a new ACL with the specified owner
    pub fn new(owner_node_id: &str) -> Self {
        Self {
            owner: owner_node_id.to_string(),
            user_rules: HashMap::new(),
            path_rules: Vec::new(),
        }
    }

    /// Get the drive owner's NodeId
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Check if a user is the owner
    pub fn is_owner(&self, node_id: &str) -> bool {
        self.owner == node_id
    }

    /// Grant access to a user
    pub fn grant(&mut self, node_id: &str, rule: AccessRule) {
        self.user_rules.insert(node_id.to_string(), rule);
    }

    /// Revoke a user's access
    pub fn revoke(&mut self, node_id: &str) -> Option<AccessRule> {
        self.user_rules.remove(node_id)
    }

    /// Add a path rule
    pub fn add_path_rule(&mut self, rule: PathRule) {
        self.path_rules.push(rule);
    }

    /// Remove a path rule by pattern
    pub fn remove_path_rule(&mut self, pattern: &str) {
        self.path_rules.retain(|r| r.pattern != pattern);
    }

    /// Get a user's base permission (ignoring path rules)
    pub fn get_user_permission(&self, node_id: &str) -> Option<Permission> {
        // Owner always has admin
        if self.is_owner(node_id) {
            return Some(Permission::Admin);
        }

        // Check user rules
        self.user_rules.get(node_id).and_then(|rule| {
            if rule.is_valid() {
                Some(rule.permission)
            } else {
                None // Expired
            }
        })
    }

    /// Check if a user has at least the required permission for a path
    pub fn check_permission(&self, node_id: &str, path: &str, required: Permission) -> bool {
        // Owner always bypasses path rules
        if self.is_owner(node_id) {
            return true;
        }

        // Get base permission
        let base_permission = match self.get_user_permission(node_id) {
            Some(p) => p,
            None => return false, // No access
        };

        // Check path rules (evaluated in order, last match wins)
        let mut effective_permission = base_permission;
        let mut denied = false;

        for rule in &self.path_rules {
            if rule.matches(path) {
                if rule.deny {
                    denied = true;
                } else {
                    denied = false;
                    // Path rule can only restrict, not elevate
                    if rule.permission < effective_permission {
                        effective_permission = rule.permission;
                    }
                }
            }
        }

        if denied {
            return false;
        }

        effective_permission.satisfies(required)
    }

    /// Get all users with access
    pub fn users(&self) -> Vec<&str> {
        let mut users: Vec<&str> = self.user_rules.keys().map(|s| s.as_str()).collect();
        if !users.contains(&self.owner.as_str()) {
            users.push(&self.owner);
        }
        users
    }

    /// Get the access rule for a specific user
    pub fn get_rule(&self, node_id: &str) -> Option<&AccessRule> {
        self.user_rules.get(node_id)
    }

    /// Get all path rules
    pub fn path_rules(&self) -> &[PathRule] {
        &self.path_rules
    }

    /// Remove expired rules
    pub fn cleanup_expired(&mut self) {
        self.user_rules.retain(|_, rule| rule.is_valid());
    }
}

/// Result of a permission check
#[derive(Debug, Clone, Serialize)]
pub struct PermissionCheckResult {
    pub allowed: bool,
    pub user_permission: Option<Permission>,
    pub effective_permission: Option<Permission>,
    pub reason: String,
}

impl PermissionCheckResult {
    pub fn allowed(permission: Permission) -> Self {
        Self {
            allowed: true,
            user_permission: Some(permission),
            effective_permission: Some(permission),
            reason: "Access granted".to_string(),
        }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            user_permission: None,
            effective_permission: None,
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_ordering() {
        assert!(Permission::Admin > Permission::Manage);
        assert!(Permission::Manage > Permission::Write);
        assert!(Permission::Write > Permission::Read);
    }

    #[test]
    fn test_permission_satisfies() {
        assert!(Permission::Admin.satisfies(Permission::Read));
        assert!(Permission::Admin.satisfies(Permission::Admin));
        assert!(!Permission::Read.satisfies(Permission::Write));
    }

    #[test]
    fn test_access_rule_expiry() {
        let rule = AccessRule::new(Permission::Read, "alice");
        assert!(!rule.is_expired());

        let expired_rule = AccessRule::new(Permission::Read, "alice")
            .with_expiry(Utc::now() - chrono::Duration::days(1));
        assert!(expired_rule.is_expired());
    }

    #[test]
    fn test_path_rule_exact_match() {
        let rule = PathRule::allow("documents/secret.txt", Permission::Read);
        assert!(rule.matches("documents/secret.txt"));
        assert!(!rule.matches("documents/other.txt"));
    }

    #[test]
    fn test_path_rule_single_star() {
        let rule = PathRule::allow("documents/*.txt", Permission::Read);
        assert!(rule.matches("documents/secret.txt"));
        assert!(rule.matches("documents/other.txt"));
        assert!(!rule.matches("images/photo.jpg"));
    }

    #[test]
    fn test_path_rule_double_star() {
        let rule = PathRule::allow("documents/**", Permission::Read);
        assert!(rule.matches("documents/secret.txt"));
        assert!(rule.matches("documents/nested/deep/file.txt"));
    }

    #[test]
    fn test_acl_owner_always_admin() {
        let acl = AccessControlList::new("owner123");
        assert_eq!(acl.get_user_permission("owner123"), Some(Permission::Admin));
        assert!(acl.check_permission("owner123", "any/path", Permission::Admin));
    }

    #[test]
    fn test_acl_grant_revoke() {
        let mut acl = AccessControlList::new("owner123");
        let rule = AccessRule::new(Permission::Write, "owner123");

        acl.grant("user456", rule);
        assert_eq!(acl.get_user_permission("user456"), Some(Permission::Write));
        assert!(acl.check_permission("user456", "file.txt", Permission::Write));
        assert!(!acl.check_permission("user456", "file.txt", Permission::Admin));

        acl.revoke("user456");
        assert_eq!(acl.get_user_permission("user456"), None);
    }

    #[test]
    fn test_acl_path_restriction() {
        let mut acl = AccessControlList::new("owner123");
        acl.grant("user456", AccessRule::new(Permission::Write, "owner123"));

        // Add path rule that restricts private folder
        acl.add_path_rule(PathRule::allow("private/**", Permission::Read));

        // User can write to normal files
        assert!(acl.check_permission("user456", "public/file.txt", Permission::Write));

        // User can only read in private folder
        assert!(acl.check_permission("user456", "private/secret.txt", Permission::Read));
        assert!(!acl.check_permission("user456", "private/secret.txt", Permission::Write));
    }

    #[test]
    fn test_acl_path_deny() {
        let mut acl = AccessControlList::new("owner123");
        acl.grant("user456", AccessRule::new(Permission::Admin, "owner123"));

        // Deny access to .git folder even for admins
        acl.add_path_rule(PathRule::deny(".git/**"));

        assert!(!acl.check_permission("user456", ".git/config", Permission::Read));
        // But owner still has access
        assert!(acl.check_permission("owner123", ".git/config", Permission::Read));
    }

    #[test]
    fn test_expired_rule_no_access() {
        let mut acl = AccessControlList::new("owner123");
        let expired_rule = AccessRule::new(Permission::Write, "owner123")
            .with_expiry(Utc::now() - chrono::Duration::days(1));

        acl.grant("user456", expired_rule);

        // Expired rule should not grant access
        assert_eq!(acl.get_user_permission("user456"), None);
        assert!(!acl.check_permission("user456", "file.txt", Permission::Read));
    }
}
