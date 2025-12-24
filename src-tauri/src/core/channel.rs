//! Backpressure-aware broadcast channel utilities
//!
//! Provides utilities for handling broadcast channels with backpressure monitoring
//! to prevent message loss and detect slow consumers.

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

/// Default warning threshold - warn when queue exceeds this many messages.
/// This is ~75% of the typical 256-message channel capacity.
const DEFAULT_WARNING_THRESHOLD: usize = 192;

/// Metrics for tracking channel health
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct ChannelMetrics {
    /// Total messages sent
    pub messages_sent: AtomicU64,
    /// Messages dropped due to no receivers
    pub messages_dropped: AtomicU64,
    /// Times channel exceeded warning threshold
    pub backpressure_warnings: AtomicU64,
}

#[allow(dead_code)]
impl ChannelMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_dropped(&self) {
        self.messages_dropped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_backpressure(&self) {
        self.backpressure_warnings.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ChannelMetricsSnapshot {
        ChannelMetricsSnapshot {
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_dropped: self.messages_dropped.load(Ordering::Relaxed),
            backpressure_warnings: self.backpressure_warnings.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of channel metrics at a point in time
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ChannelMetricsSnapshot {
    pub messages_sent: u64,
    pub messages_dropped: u64,
    pub backpressure_warnings: u64,
}

/// Send a message with backpressure monitoring.
///
/// Logs a warning when the channel queue length exceeds the warning threshold,
/// indicating that consumers may be falling behind.
///
/// # Arguments
/// * `tx` - The broadcast sender
/// * `msg` - The message to send
/// * `channel_name` - Name for logging purposes
///
/// # Returns
/// Returns `true` if the message was sent (even if no receivers).
pub fn send_with_backpressure<T: Clone>(
    tx: &broadcast::Sender<T>,
    msg: T,
    channel_name: &str,
) -> bool {
    let current_len = tx.len();

    if current_len >= DEFAULT_WARNING_THRESHOLD {
        tracing::warn!(
            channel = channel_name,
            queue_length = current_len,
            threshold = DEFAULT_WARNING_THRESHOLD,
            "Channel backpressure detected - consumers may be falling behind"
        );
    }

    match tx.send(msg) {
        Ok(receiver_count) => {
            tracing::trace!(
                channel = channel_name,
                receivers = receiver_count,
                "Message sent successfully"
            );
            true
        }
        Err(_) => {
            // No active receivers - message is dropped
            tracing::debug!(
                channel = channel_name,
                "Message dropped - no active receivers"
            );
            true // Still "successful" in that it didn't error
        }
    }
}

/// Send a message with backpressure monitoring and metrics tracking.
///
/// Like `send_with_backpressure` but also updates metrics counters.
#[allow(dead_code)]
pub fn send_with_metrics<T: Clone>(
    tx: &broadcast::Sender<T>,
    msg: T,
    channel_name: &str,
    metrics: &ChannelMetrics,
) -> bool {
    let current_len = tx.len();

    if current_len >= DEFAULT_WARNING_THRESHOLD {
        tracing::warn!(
            channel = channel_name,
            queue_length = current_len,
            threshold = DEFAULT_WARNING_THRESHOLD,
            "Channel backpressure detected - consumers may be falling behind"
        );
        metrics.record_backpressure();
    }

    match tx.send(msg) {
        Ok(receiver_count) => {
            metrics.record_sent();
            tracing::trace!(
                channel = channel_name,
                receivers = receiver_count,
                "Message sent successfully"
            );
            true
        }
        Err(_) => {
            metrics.record_dropped();
            tracing::debug!(
                channel = channel_name,
                "Message dropped - no active receivers"
            );
            true
        }
    }
}

/// Check if a channel is experiencing backpressure.
#[allow(dead_code)]
pub fn is_under_pressure<T>(tx: &broadcast::Sender<T>) -> bool {
    tx.len() >= DEFAULT_WARNING_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_with_backpressure_normal() {
        let (tx, mut rx) = broadcast::channel::<i32>(16);

        let result = send_with_backpressure(&tx, 42, "test");
        assert!(result);

        let received = rx.try_recv().unwrap();
        assert_eq!(received, 42);
    }

    #[test]
    fn test_send_no_receivers() {
        let (tx, _) = broadcast::channel::<i32>(16);

        // Should not panic even with no receivers
        let result = send_with_backpressure(&tx, 42, "test");
        assert!(result);
    }

    #[test]
    fn test_is_under_pressure() {
        let (tx, _rx) = broadcast::channel::<i32>(256);

        // Fill below threshold
        for i in 0..100 {
            let _ = tx.send(i);
        }
        assert!(!is_under_pressure(&tx));

        // Fill above threshold (192)
        for i in 100..200 {
            let _ = tx.send(i);
        }
        assert!(is_under_pressure(&tx));
    }

    #[test]
    fn test_metrics_tracking() {
        let (tx, _rx) = broadcast::channel::<i32>(16);
        let metrics = ChannelMetrics::new();

        send_with_metrics(&tx, 1, "test", &metrics);
        send_with_metrics(&tx, 2, "test", &metrics);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages_sent, 2);
        assert_eq!(snapshot.messages_dropped, 0);
    }
}
