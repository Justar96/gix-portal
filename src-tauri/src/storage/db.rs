use anyhow::Result;
use redb::{Database as RedbDatabase, ReadableTable, TableDefinition};
use std::path::Path;

// Table definitions
const IDENTITY_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("identity");
const DRIVES_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("drives");

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
}
