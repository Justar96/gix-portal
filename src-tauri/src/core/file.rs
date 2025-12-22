use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use walkdir::WalkDir;

/// Represents a file or directory entry
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntry {
    /// File name
    pub name: String,
    /// Relative path from drive root
    pub path: PathBuf,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Size in bytes (0 for directories)
    pub size: u64,
    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
}

/// DTO for sending file entry to frontend
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntryDto {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: String,
}

impl From<&FileEntry> for FileEntryDto {
    fn from(entry: &FileEntry) -> Self {
        Self {
            name: entry.name.clone(),
            path: entry.path.to_string_lossy().to_string(),
            is_dir: entry.is_dir,
            size: entry.size,
            modified_at: entry.modified_at.to_rfc3339(),
        }
    }
}

/// Index a directory recursively and return all file entries
pub fn index_directory(root: &std::path::Path) -> anyhow::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(root)
        .min_depth(1) // Skip the root directory itself
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden files and common ignored directories
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "__pycache__"
                && name != ".git"
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // Skip entries we can't read
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let relative_path = match entry.path().strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };

        let modified = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: relative_path,
            is_dir: metadata.is_dir(),
            size: if metadata.is_file() { metadata.len() } else { 0 },
            modified_at: modified,
        });
    }

    Ok(entries)
}

/// List files in a specific directory (non-recursive)
pub fn list_directory(root: &std::path::Path, subpath: &str) -> anyhow::Result<Vec<FileEntry>> {
    let target = if subpath.is_empty() || subpath == "/" {
        root.to_path_buf()
    } else {
        root.join(subpath.trim_start_matches('/'))
    };

    if !target.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {:?}", target));
    }

    if !target.is_dir() {
        return Err(anyhow::anyhow!("Path is not a directory: {:?}", target));
    }

    let mut entries = Vec::new();

    for entry in std::fs::read_dir(&target)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        let relative_path = match entry.path().strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };

        let modified = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());

        entries.push(FileEntry {
            name,
            path: relative_path,
            is_dir: metadata.is_dir(),
            size: if metadata.is_file() { metadata.len() } else { 0 },
            modified_at: modified,
        });
    }

    // Sort: directories first, then by name (case-insensitive)
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}
