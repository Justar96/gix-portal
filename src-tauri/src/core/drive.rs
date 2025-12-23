use crate::crypto::NodeId;
use blake3::Hasher;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique drive identifier (32-byte BLAKE3 hash)
#[derive(Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct DriveId(pub [u8; 32]);

impl DriveId {
    /// Generate DriveId from owner, path, and current timestamp
    pub fn generate(owner: &NodeId, path: &std::path::Path) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(owner.as_bytes());
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(&Utc::now().timestamp_millis().to_le_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    #[allow(dead_code)]
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl std::fmt::Display for DriveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// A shared drive represents a folder that can be accessed by multiple peers
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SharedDrive {
    /// Unique identifier for this drive
    pub id: DriveId,
    /// Human-readable name
    pub name: String,
    /// Local path on owner's machine
    pub local_path: PathBuf,
    /// Owner's public key
    pub owner: NodeId,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Total size in bytes (calculated from file index)
    pub total_size: u64,
    /// Number of files (calculated from file index)
    pub file_count: u64,
}

impl SharedDrive {
    /// Create a new shared drive
    pub fn new(name: String, local_path: PathBuf, owner: NodeId) -> Self {
        let id = DriveId::generate(&owner, &local_path);
        Self {
            id,
            name,
            local_path,
            owner,
            created_at: Utc::now(),
            total_size: 0,
            file_count: 0,
        }
    }

    /// Update statistics after indexing
    pub fn update_stats(&mut self, total_size: u64, file_count: u64) {
        self.total_size = total_size;
        self.file_count = file_count;
    }
}

/// DTO for sending drive info to frontend
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DriveInfo {
    pub id: String,
    pub name: String,
    pub local_path: String,
    pub owner: String,
    pub created_at: String,
    pub total_size: u64,
    pub file_count: u64,
}

impl From<&SharedDrive> for DriveInfo {
    fn from(drive: &SharedDrive) -> Self {
        Self {
            id: drive.id.to_hex(),
            name: drive.name.clone(),
            local_path: drive.local_path.to_string_lossy().to_string(),
            owner: drive.owner.to_hex(),
            created_at: drive.created_at.to_rfc3339(),
            total_size: drive.total_size,
            file_count: drive.file_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Identity;

    #[test]
    fn test_drive_id_generation() {
        let identity = Identity::generate();
        let node_id = identity.node_id();
        let path = std::path::Path::new("/test/path");

        let id1 = DriveId::generate(&node_id, path);
        // Sleep to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = DriveId::generate(&node_id, path);

        // IDs should be unique due to timestamp
        assert_ne!(id1.as_bytes(), id2.as_bytes());
    }

    #[test]
    fn test_drive_id_hex() {
        let identity = Identity::generate();
        let node_id = identity.node_id();
        let path = std::path::Path::new("/test/path");

        let id = DriveId::generate(&node_id, path);
        let hex_str = id.to_hex();
        let restored = DriveId::from_hex(&hex_str).unwrap();

        assert_eq!(id.as_bytes(), restored.as_bytes());
    }
}
