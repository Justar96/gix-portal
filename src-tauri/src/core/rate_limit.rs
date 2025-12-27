//! Rate limiting for abuse prevention
//!
//! Implements token bucket rate limiting for critical operations.
//! Prevents abuse of invite generation, file uploads, and other sensitive APIs.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Configuration for a rate limit bucket
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum tokens in the bucket
    pub max_tokens: u32,
    /// Tokens refilled per second
    pub refill_rate: f64,
    /// Initial tokens (defaults to max_tokens)
    pub initial_tokens: Option<u32>,
}

impl RateLimitConfig {
    /// Create a new rate limit config
    pub fn new(max_tokens: u32, refill_per_second: f64) -> Self {
        Self {
            max_tokens,
            refill_rate: refill_per_second,
            initial_tokens: None,
        }
    }

    /// Preset for invite generation (10 per minute)
    pub fn invite_generation() -> Self {
        Self::new(10, 10.0 / 60.0)
    }

    /// Preset for file uploads (100 per minute)
    pub fn file_upload() -> Self {
        Self::new(100, 100.0 / 60.0)
    }

    /// Preset for file downloads (200 per minute)
    pub fn file_download() -> Self {
        Self::new(200, 200.0 / 60.0)
    }

    /// Preset for API calls (1000 per minute)
    pub fn general_api() -> Self {
        Self::new(1000, 1000.0 / 60.0)
    }

    /// Preset for drive creation (5 per minute)
    pub fn drive_creation() -> Self {
        Self::new(5, 5.0 / 60.0)
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self::general_api()
    }
}

/// A token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    max_tokens: u32,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        Self {
            tokens: config.initial_tokens.unwrap_or(config.max_tokens) as f64,
            max_tokens: config.max_tokens,
            refill_rate: config.refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens as f64);
        self.last_refill = now;
    }

    /// Try to consume tokens, returns true if successful
    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        if self.tokens >= tokens as f64 {
            self.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    /// Get time until tokens are available
    fn time_until_available(&mut self, tokens: u32) -> Duration {
        self.refill();
        if self.tokens >= tokens as f64 {
            Duration::ZERO
        } else if self.refill_rate <= 0.0 {
            // No refill - return a large duration (1 hour)
            Duration::from_secs(3600)
        } else {
            let needed = tokens as f64 - self.tokens;
            Duration::from_secs_f64(needed / self.refill_rate)
        }
    }

    /// Get current token count
    fn available_tokens(&mut self) -> u32 {
        self.refill();
        self.tokens as u32
    }
}

/// Rate limit result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request allowed
    Allowed { remaining: u32 },
    /// Request denied - must wait
    Denied { retry_after: Duration },
}

impl RateLimitResult {
    #[allow(dead_code)]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

/// Operation types for rate limiting
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RateLimitOperation {
    InviteGeneration,
    #[allow(dead_code)]
    FileUpload,
    #[allow(dead_code)]
    FileDownload,
    #[allow(dead_code)]
    DriveCreation,
    #[allow(dead_code)]
    GeneralApi,
    #[allow(dead_code)]
    Custom(String),
}

impl RateLimitOperation {
    fn default_config(&self) -> RateLimitConfig {
        match self {
            Self::InviteGeneration => RateLimitConfig::invite_generation(),
            Self::FileUpload => RateLimitConfig::file_upload(),
            Self::FileDownload => RateLimitConfig::file_download(),
            Self::DriveCreation => RateLimitConfig::drive_creation(),
            Self::GeneralApi | Self::Custom(_) => RateLimitConfig::general_api(),
        }
    }
}

/// Per-identity rate limiter
struct IdentityRateLimiter {
    buckets: HashMap<RateLimitOperation, TokenBucket>,
}

impl IdentityRateLimiter {
    fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    fn get_or_create_bucket(
        &mut self,
        operation: &RateLimitOperation,
        custom_config: Option<&RateLimitConfig>,
    ) -> &mut TokenBucket {
        self.buckets
            .entry(operation.clone())
            .or_insert_with(|| match custom_config {
                Some(config) => TokenBucket::new(config),
                None => TokenBucket::new(&operation.default_config()),
            })
    }
}

/// Global rate limiter manager
pub struct RateLimiter {
    /// Per-identity rate limiters
    limiters: RwLock<HashMap<[u8; 32], IdentityRateLimiter>>,
    /// Custom configs for operations
    configs: RwLock<HashMap<RateLimitOperation, RateLimitConfig>>,
    /// Whether rate limiting is enabled
    enabled: bool,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new() -> Self {
        Self {
            limiters: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
            enabled: true,
        }
    }

    /// Create a disabled rate limiter (for testing)
    #[allow(dead_code)]
    pub fn disabled() -> Self {
        Self {
            limiters: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
            enabled: false,
        }
    }

    /// Set custom config for an operation
    #[allow(dead_code)]
    pub async fn set_config(&self, operation: RateLimitOperation, config: RateLimitConfig) {
        // Clear existing limiters to force new buckets with new config
        let mut limiters = self.limiters.write().await;
        limiters.clear();
        drop(limiters);

        let mut configs = self.configs.write().await;
        configs.insert(operation, config);
    }

    /// Check and consume rate limit
    pub async fn check(
        &self,
        identity: &[u8; 32],
        operation: RateLimitOperation,
    ) -> RateLimitResult {
        self.check_consume(identity, operation, 1).await
    }

    /// Check and consume multiple tokens
    pub async fn check_consume(
        &self,
        identity: &[u8; 32],
        operation: RateLimitOperation,
        tokens: u32,
    ) -> RateLimitResult {
        if !self.enabled {
            return RateLimitResult::Allowed {
                remaining: u32::MAX,
            };
        }

        // Get custom config if any
        let configs = self.configs.read().await;
        let custom_config = configs.get(&operation).cloned();
        drop(configs);

        let mut limiters = self.limiters.write().await;
        let limiter = limiters
            .entry(*identity)
            .or_insert_with(IdentityRateLimiter::new);
        let bucket = limiter.get_or_create_bucket(&operation, custom_config.as_ref());

        if bucket.try_consume(tokens) {
            RateLimitResult::Allowed {
                remaining: bucket.available_tokens(),
            }
        } else {
            RateLimitResult::Denied {
                retry_after: bucket.time_until_available(tokens),
            }
        }
    }

    /// Get remaining tokens without consuming
    #[allow(dead_code)]
    pub async fn remaining(&self, identity: &[u8; 32], operation: RateLimitOperation) -> u32 {
        if !self.enabled {
            return u32::MAX;
        }

        // Get custom config if any
        let configs = self.configs.read().await;
        let custom_config = configs.get(&operation).cloned();
        drop(configs);

        let mut limiters = self.limiters.write().await;
        let limiter = limiters
            .entry(*identity)
            .or_insert_with(IdentityRateLimiter::new);
        let bucket = limiter.get_or_create_bucket(&operation, custom_config.as_ref());
        bucket.available_tokens()
    }

    /// Clean up old entries (identities not seen recently)
    #[allow(dead_code)]
    pub async fn cleanup_stale(&self, _max_age: Duration) {
        // For simplicity, we'll just clear all entries
        // In production, you'd track last access time per identity
        let mut limiters = self.limiters.write().await;
        if limiters.len() > 10000 {
            // Only cleanup if we have many entries
            limiters.clear();
            tracing::info!("Cleared rate limiter cache");
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Arc wrapper for convenient sharing
pub type SharedRateLimiter = Arc<RateLimiter>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_allowed() {
        let limiter = RateLimiter::new();
        let identity = [0u8; 32];

        // Should allow first request
        let result = limiter
            .check(&identity, RateLimitOperation::GeneralApi)
            .await;
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_rate_limit_denied_after_exhaustion() {
        let limiter = RateLimiter::new();
        let identity = [1u8; 32];

        // Set very restrictive limit
        limiter
            .set_config(
                RateLimitOperation::Custom("test".to_string()),
                RateLimitConfig::new(2, 0.0), // 2 tokens, no refill
            )
            .await;

        let op = RateLimitOperation::Custom("test".to_string());

        // First two should succeed
        assert!(limiter.check(&identity, op.clone()).await.is_allowed());
        assert!(limiter.check(&identity, op.clone()).await.is_allowed());

        // Third should be denied
        let result = limiter.check(&identity, op.clone()).await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_rate_limit_disabled() {
        let limiter = RateLimiter::disabled();
        let identity = [2u8; 32];

        // Should always allow when disabled
        for _ in 0..1000 {
            assert!(limiter
                .check(&identity, RateLimitOperation::InviteGeneration)
                .await
                .is_allowed());
        }
    }

    #[tokio::test]
    async fn test_per_identity_isolation() {
        let limiter = RateLimiter::new();
        let identity1 = [3u8; 32];
        let identity2 = [4u8; 32];

        limiter
            .set_config(
                RateLimitOperation::Custom("isolated".to_string()),
                RateLimitConfig::new(1, 0.0),
            )
            .await;

        let op = RateLimitOperation::Custom("isolated".to_string());

        // Both identities should get their own bucket
        assert!(limiter.check(&identity1, op.clone()).await.is_allowed());
        assert!(limiter.check(&identity2, op.clone()).await.is_allowed());

        // But second request for each should fail
        assert!(!limiter.check(&identity1, op.clone()).await.is_allowed());
        assert!(!limiter.check(&identity2, op.clone()).await.is_allowed());
    }

    #[tokio::test]
    async fn test_token_refill() {
        let limiter = RateLimiter::new();
        let identity = [5u8; 32];

        limiter
            .set_config(
                RateLimitOperation::Custom("refill".to_string()),
                RateLimitConfig::new(1, 1000.0), // 1 token, 1000 per second refill
            )
            .await;

        let op = RateLimitOperation::Custom("refill".to_string());

        // Exhaust token
        assert!(limiter.check(&identity, op.clone()).await.is_allowed());
        assert!(!limiter.check(&identity, op.clone()).await.is_allowed());

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should have tokens again
        assert!(limiter.check(&identity, op.clone()).await.is_allowed());
    }
}
