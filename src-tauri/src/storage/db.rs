use anyhow::Result;
use redb::{Database as RedbDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::path::Path;

// Table definitions
const IDENTITY_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("identity");
const DRIVES_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("drives");
const ACLS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("acls");
const TOKEN_TRACKERS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("token_trackers");
const KEY_EXCHANGE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("key_exchange");
const DRIVE_KEYS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("drive_keys");
const AUDIT_LOG_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("audit_log");
const AUDIT_COUNTER_TABLE: TableDefinition<&str, u64> = TableDefinition::new("audit_counter");
const REVOKED_TOKENS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("revoked_tokens");
/// File metadata table - key: "drive_id:file_path", value: serialized FileMetadata
const FILE_METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("file_metadata");

/// Database wrapper for persistent storage using redb
pub struct Database {
    db: RedbDatabase,
}

impl Database {
    /// Open or create database at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = RedbDatabase::create(path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(IDENTITY_TABLE)?;
            let _ = write_txn.open_table(DRIVES_TABLE)?;
            let _ = write_txn.open_table(ACLS_TABLE)?;
            let _ = write_txn.open_table(TOKEN_TRACKERS_TABLE)?;
            let _ = write_txn.open_table(KEY_EXCHANGE_TABLE)?;
            let _ = write_txn.open_table(DRIVE_KEYS_TABLE)?;
            let _ = write_txn.open_table(AUDIT_LOG_TABLE)?;
            let _ = write_txn.open_table(AUDIT_COUNTER_TABLE)?;
            let _ = write_txn.open_table(REVOKED_TOKENS_TABLE)?;
            let _ = write_txn.open_table(FILE_METADATA_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    /// Get stored identity secret key bytes
    pub fn get_identity(&self) -> Result<Option<[u8; 32]>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(IDENTITY_TABLE)?;

        match table.get("secret_key")? {
            Some(guard) => {
                let bytes = guard.value();
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(bytes);
                    Ok(Some(arr))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Save identity secret key
    pub fn save_identity(&self, secret_key: &[u8; 32]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(IDENTITY_TABLE)?;
            table.insert("secret_key", secret_key.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Save a drive to the database
    pub fn save_drive(&self, drive_id: &[u8; 32], data: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(DRIVES_TABLE)?;
            table.insert(drive_id.as_slice(), data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a specific drive by ID
    #[allow(dead_code)]
    pub fn get_drive(&self, drive_id: &[u8; 32]) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DRIVES_TABLE)?;

        match table.get(drive_id.as_slice())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Load all drives from database
    pub fn list_drives(&self) -> Result<Vec<([u8; 32], Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DRIVES_TABLE)?;

        let mut drives = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let key_bytes = key.value();
            if key_bytes.len() == 32 {
                let mut id = [0u8; 32];
                id.copy_from_slice(key_bytes);
                drives.push((id, value.value().to_vec()));
            }
        }
        Ok(drives)
    }

    /// Delete a drive from database
    #[allow(dead_code)]
    pub fn delete_drive(&self, drive_id: &[u8; 32]) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(DRIVES_TABLE)?;
            let result = table.remove(drive_id.as_slice())?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    // ============================================================================
    // ACL Operations
    // ============================================================================

    /// Save an ACL for a drive
    pub fn save_acl(&self, drive_id: &str, data: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ACLS_TABLE)?;
            table.insert(drive_id, data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get an ACL for a drive
    pub fn get_acl(&self, drive_id: &str) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACLS_TABLE)?;

        match table.get(drive_id)? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Load all ACLs from database
    pub fn list_acls(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ACLS_TABLE)?;

        let mut acls = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            acls.push((key.value().to_string(), value.value().to_vec()));
        }
        Ok(acls)
    }

    /// Delete an ACL
    pub fn delete_acl(&self, drive_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(ACLS_TABLE)?;
            let result = table.remove(drive_id)?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    // ============================================================================
    // Token Tracker Operations
    // ============================================================================

    /// Save a token tracker for a drive
    pub fn save_token_tracker(&self, drive_id: &str, data: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TOKEN_TRACKERS_TABLE)?;
            table.insert(drive_id, data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a token tracker for a drive
    #[allow(dead_code)]
    pub fn get_token_tracker(&self, drive_id: &str) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TOKEN_TRACKERS_TABLE)?;

        match table.get(drive_id)? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Load all token trackers from database
    pub fn list_token_trackers(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TOKEN_TRACKERS_TABLE)?;

        let mut trackers = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            trackers.push((key.value().to_string(), value.value().to_vec()));
        }
        Ok(trackers)
    }

    // ============================================================================
    // Key Exchange Operations
    // ============================================================================

    /// Save the key exchange keypair secret key
    pub fn save_key_exchange_keypair(&self, secret_key: &[u8; 32]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(KEY_EXCHANGE_TABLE)?;
            table.insert("secret_key", secret_key.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get the key exchange keypair secret key
    pub fn get_key_exchange_keypair(&self) -> Result<Option<[u8; 32]>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(KEY_EXCHANGE_TABLE)?;

        match table.get("secret_key")? {
            Some(guard) => {
                let bytes = guard.value();
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(bytes);
                    Ok(Some(arr))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    // ============================================================================
    // Drive Key Operations
    // ============================================================================

    /// Save an encrypted drive key
    pub fn save_drive_key(&self, drive_id: &str, wrapped_key: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(DRIVE_KEYS_TABLE)?;
            table.insert(drive_id, wrapped_key)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get an encrypted drive key
    pub fn get_drive_key(&self, drive_id: &str) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(DRIVE_KEYS_TABLE)?;

        match table.get(drive_id)? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Delete a drive key
    #[allow(dead_code)]
    pub fn delete_drive_key(&self, drive_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(DRIVE_KEYS_TABLE)?;
            let result = table.remove(drive_id)?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    // ============================================================================
    // Audit Log Operations
    // ============================================================================

    /// Append an audit log entry and return the assigned ID
    pub fn append_audit_log(&self, data: &[u8]) -> Result<u64> {
        let write_txn = self.db.begin_write()?;
        let id = {
            // Get and increment counter
            let mut counter_table = write_txn.open_table(AUDIT_COUNTER_TABLE)?;
            let current_id = counter_table
                .get("next_id")?
                .map(|v| v.value())
                .unwrap_or(1);
            counter_table.insert("next_id", current_id + 1)?;

            // Insert audit entry
            let mut audit_table = write_txn.open_table(AUDIT_LOG_TABLE)?;
            audit_table.insert(current_id, data)?;

            current_id
        };
        write_txn.commit()?;
        Ok(id)
    }

    /// Query audit log entries with pagination
    pub fn query_audit_log(
        &self,
        since_ms: Option<i64>,
        until_ms: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(u64, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(AUDIT_LOG_TABLE)?;

        let mut entries = Vec::new();
        let mut skipped = 0;

        // Iterate in reverse order (newest first)
        for entry in table.iter()?.rev() {
            let (key, value) = entry?;
            let id = key.value();
            let data = value.value().to_vec();

            // Apply timestamp filters if data contains timestamp
            // This is a simple filter - entries are stored with timestamp in JSON
            if since_ms.is_some() || until_ms.is_some() {
                // Parse timestamp from JSON if possible
                if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&data) {
                    if let Some(ts) = parsed.get("timestamp").and_then(|t| t.as_str()) {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                            let ts_ms = dt.timestamp_millis();
                            if let Some(since) = since_ms {
                                if ts_ms < since {
                                    continue;
                                }
                            }
                            if let Some(until) = until_ms {
                                if ts_ms > until {
                                    continue;
                                }
                            }
                        }
                    }
                }
            }

            // Apply pagination
            if skipped < offset {
                skipped += 1;
                continue;
            }

            entries.push((id, data));

            if entries.len() >= limit {
                break;
            }
        }

        Ok(entries)
    }

    /// Count total audit log entries
    pub fn count_audit_log(&self) -> Result<u64> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(AUDIT_LOG_TABLE)?;
        Ok(table.len()?)
    }

    // ============================================================================
    // Revoked Tokens Operations
    // ============================================================================

    /// Save revoked tokens for a drive
    pub fn save_revoked_tokens(&self, drive_id: &str, data: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(REVOKED_TOKENS_TABLE)?;
            table.insert(drive_id, data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get revoked tokens for a drive
    pub fn get_revoked_tokens(&self, drive_id: &str) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(REVOKED_TOKENS_TABLE)?;

        match table.get(drive_id)? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Load all revoked tokens from database
    pub fn list_revoked_tokens(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(REVOKED_TOKENS_TABLE)?;

        let mut tokens = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            tokens.push((key.value().to_string(), value.value().to_vec()));
        }
        Ok(tokens)
    }

    // ============ File Metadata Operations ============

    /// Generate a key for file metadata: "drive_id:path"
    fn file_metadata_key(drive_id: &str, path: &str) -> String {
        format!("{}:{}", drive_id, path)
    }

    /// Save file metadata for a specific file in a drive
    pub fn save_file_metadata(&self, drive_id: &str, path: &str, data: &[u8]) -> Result<()> {
        let key = Self::file_metadata_key(drive_id, path);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(FILE_METADATA_TABLE)?;
            table.insert(key.as_str(), data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get file metadata for a specific file
    pub fn get_file_metadata(&self, drive_id: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let key = Self::file_metadata_key(drive_id, path);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(FILE_METADATA_TABLE)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Delete file metadata for a specific file
    pub fn delete_file_metadata(&self, drive_id: &str, path: &str) -> Result<()> {
        let key = Self::file_metadata_key(drive_id, path);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(FILE_METADATA_TABLE)?;
            table.remove(key.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// List all file metadata for a drive (returns path and serialized metadata)
    pub fn list_file_metadata(&self, drive_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let prefix = format!("{}:", drive_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(FILE_METADATA_TABLE)?;

        let mut metadata = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let key_str = key.value();
            if key_str.starts_with(&prefix) {
                // Extract path from key (remove "drive_id:" prefix)
                let path = key_str[prefix.len()..].to_string();
                metadata.push((path, value.value().to_vec()));
            }
        }
        Ok(metadata)
    }

    /// Delete all file metadata for a drive
    pub fn delete_drive_metadata(&self, drive_id: &str) -> Result<usize> {
        let prefix = format!("{}:", drive_id);
        let write_txn = self.db.begin_write()?;
        let mut deleted = 0;
        {
            let mut table = write_txn.open_table(FILE_METADATA_TABLE)?;
            
            // Collect keys to delete
            let keys_to_delete: Vec<String> = {
                let read_table = write_txn.open_table(FILE_METADATA_TABLE)?;
                read_table.iter()?
                    .filter_map(|entry| {
                        entry.ok().and_then(|(key, _)| {
                            let key_str = key.value().to_string();
                            if key_str.starts_with(&prefix) {
                                Some(key_str)
                            } else {
                                None
                            }
                        })
                    })
                    .collect()
            };

            for key in keys_to_delete {
                table.remove(key.as_str())?;
                deleted += 1;
            }
        }
        write_txn.commit()?;
        Ok(deleted)
    }
}
