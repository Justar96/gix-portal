// Allow dead code in crypto modules - these are APIs for future integration
#[allow(dead_code)]
pub mod access;
#[allow(dead_code)]
pub mod encryption;
#[allow(dead_code)]
pub mod encryption_manager;
#[allow(dead_code)]
pub mod invite;
#[allow(dead_code)]
pub mod key_exchange;
pub mod keys;

// Re-export commonly used types
pub use access::{AccessControlList, AccessRule, Permission};
pub use encryption::{DriveEncryption, DriveKey, EncryptionError};
pub use encryption_manager::EncryptionManager;
pub use invite::{InviteBuilder, InviteToken, TokenTracker};
pub use key_exchange::{KeyExchangeError, KeyExchangePair, WrappedKey};
pub use keys::{Identity, NodeId};
