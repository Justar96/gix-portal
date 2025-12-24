//! Background cleanup tasks for expired resources
//!
//! Provides periodic cleanup of:
//! - Expired file locks
//! - Old activity entries
//! - Expired ACL rules
//! - Stale presence data

use crate::commands::SecurityStore;
use crate::core::{ConflictManager, LockManager, PresenceManager};
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::time::{interval, Duration as TokioDuration};

/// Configuration for cleanup intervals
pub struct CleanupConfig {
    /// How often to run cleanup (in seconds)
    pub interval_secs: u64,
    /// Max age for activity entries (in hours)
    pub max_activity_age_hours: i64,
    /// Max age for resolved conflicts (in days)
    pub max_resolved_conflict_age_days: i64,
    /// Idle threshold for presence (in minutes)
    pub presence_idle_threshold_mins: i64,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300,          // 5 minutes
            max_activity_age_hours: 168, // 1 week
            max_resolved_conflict_age_days: 30,
            presence_idle_threshold_mins: 15,
        }
    }
}

/// Cleanup manager that runs periodic maintenance tasks
pub struct CleanupManager {
    config: CleanupConfig,
}

impl CleanupManager {
    pub fn new() -> Self {
        Self {
            config: CleanupConfig::default(),
        }
    }

    pub fn with_config(config: CleanupConfig) -> Self {
        Self { config }
    }

    /// Start the background cleanup task
    ///
    /// This spawns a tokio task that runs cleanup periodically.
    /// Returns a handle that can be used to abort the task.
    pub fn start(
        &self,
        lock_manager: Arc<LockManager>,
        conflict_manager: Arc<ConflictManager>,
        presence_manager: Arc<PresenceManager>,
        security_store: Arc<SecurityStore>,
    ) -> tauri::async_runtime::JoinHandle<()> {
        let interval_secs = self.config.interval_secs;
        let max_activity_age = Duration::hours(self.config.max_activity_age_hours);
        let max_resolved_age = Duration::days(self.config.max_resolved_conflict_age_days);
        let idle_threshold = Duration::minutes(self.config.presence_idle_threshold_mins);

        tauri::async_runtime::spawn(async move {
            let mut ticker = interval(TokioDuration::from_secs(interval_secs));

            tracing::info!(interval_secs = interval_secs, "Cleanup manager started");

            loop {
                ticker.tick().await;

                let start = std::time::Instant::now();
                let mut cleaned = CleanupStats::default();

                // Cleanup expired locks
                cleaned.locks = cleanup_expired_locks(&lock_manager).await;

                // Cleanup old activities
                cleaned.activities =
                    cleanup_old_activities(&presence_manager, max_activity_age).await;

                // Cleanup stale presence
                cleaned.presence = cleanup_stale_presence(&presence_manager, idle_threshold).await;

                // Cleanup old resolved conflicts
                cleaned.conflicts =
                    cleanup_old_conflicts(&conflict_manager, max_resolved_age).await;

                // Cleanup expired ACL rules
                cleaned.acl_rules = cleanup_expired_acls(&security_store).await;

                let elapsed = start.elapsed();

                if cleaned.total() > 0 {
                    tracing::info!(
                        locks = cleaned.locks,
                        activities = cleaned.activities,
                        presence = cleaned.presence,
                        conflicts = cleaned.conflicts,
                        acl_rules = cleaned.acl_rules,
                        elapsed_ms = elapsed.as_millis(),
                        "Cleanup completed"
                    );
                } else {
                    tracing::debug!(
                        elapsed_ms = elapsed.as_millis(),
                        "Cleanup completed - nothing to clean"
                    );
                }
            }
        })
    }
}

impl Default for CleanupManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about what was cleaned up
#[derive(Default)]
struct CleanupStats {
    locks: usize,
    activities: usize,
    presence: usize,
    conflicts: usize,
    acl_rules: usize,
}

impl CleanupStats {
    fn total(&self) -> usize {
        self.locks + self.activities + self.presence + self.conflicts + self.acl_rules
    }
}

/// Cleanup expired file locks across all drives
async fn cleanup_expired_locks(lock_manager: &Arc<LockManager>) -> usize {
    lock_manager.cleanup_expired().await
}

/// Cleanup old activity entries
async fn cleanup_old_activities(
    presence_manager: &Arc<PresenceManager>,
    max_age: Duration,
) -> usize {
    let cutoff = Utc::now() - max_age;
    presence_manager.cleanup_old_activities(cutoff).await
}

/// Mark stale users as away/offline
async fn cleanup_stale_presence(
    presence_manager: &Arc<PresenceManager>,
    idle_threshold: Duration,
) -> usize {
    presence_manager.update_idle_status(idle_threshold).await
}

/// Cleanup old resolved conflicts
async fn cleanup_old_conflicts(
    conflict_manager: &Arc<ConflictManager>,
    max_age: Duration,
) -> usize {
    let cutoff = Utc::now() - max_age;
    conflict_manager.cleanup_old_resolved(cutoff).await
}

/// Cleanup expired ACL rules
async fn cleanup_expired_acls(security_store: &Arc<SecurityStore>) -> usize {
    security_store.cleanup_expired().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CleanupConfig::default();
        assert_eq!(config.interval_secs, 300);
        assert_eq!(config.max_activity_age_hours, 168);
    }

    #[test]
    fn test_cleanup_stats() {
        let stats = CleanupStats {
            locks: 5,
            activities: 10,
            presence: 2,
            conflicts: 1,
            acl_rules: 3,
        };
        assert_eq!(stats.total(), 21);
    }
}
