// Allow dead code for APIs designed for future use
pub mod audit;
pub mod channel;
pub mod cleanup;
#[allow(dead_code)]
pub mod conflict;
pub mod drive;
pub mod error;
pub mod events;
pub mod file;
pub mod identity;
#[allow(dead_code)]
pub mod locking;
#[allow(dead_code)]
pub mod presence;
pub mod rate_limit;
pub mod validation;
pub mod watcher;

pub use audit::{AuditEntryDto, AuditFilter, AuditLogger};
pub use channel::send_with_backpressure;
pub use cleanup::CleanupManager;
pub use conflict::{ConflictManager, FileConflictDto, ResolutionStrategy};
pub use drive::{DriveId, DriveInfo, SharedDrive};
pub use error::AppError;
pub use events::{DriveEvent, DriveEventDto, SignedGossipMessage};
pub use file::FileEntryDto;
pub use identity::IdentityManager;
pub use locking::{FileLock, FileLockDto, LockManager, LockResult, LockType};
pub use presence::{ActivityEntryDto, PresenceManager, UserPresenceDto};
pub use rate_limit::{RateLimiter, SharedRateLimiter};
pub use validation::{validate_drive_id, validate_name, validate_path};
pub use watcher::FileWatcherManager;
