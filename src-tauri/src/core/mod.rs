// Allow dead code for APIs designed for future use
#[allow(dead_code)]
pub mod conflict;
pub mod drive;
pub mod events;
pub mod file;
pub mod identity;
#[allow(dead_code)]
pub mod locking;
#[allow(dead_code)]
pub mod presence;
pub mod watcher;

pub use conflict::{ConflictManager, FileConflictDto, ResolutionStrategy};
pub use drive::{DriveId, DriveInfo, SharedDrive};
pub use events::{DriveEvent, DriveEventDto};
pub use file::FileEntryDto;
pub use identity::IdentityManager;
pub use locking::{FileLock, FileLockDto, LockManager, LockResult, LockType};
pub use presence::{ActivityEntryDto, PresenceManager, UserPresenceDto};
pub use watcher::FileWatcherManager;
