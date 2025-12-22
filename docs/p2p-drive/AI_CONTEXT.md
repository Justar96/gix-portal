# P2P Drive Share - AI Coding Assistant Context

> This document provides context for AI coding assistants (Cursor, Copilot, Claude, etc.) working on this project.

## Project Overview

**Name:** P2P Realtime Drive Sharing System  
**Type:** Desktop application (Tauri v2)  
**Stack:** Rust backend + React frontend  
**Purpose:** Self-hosted peer-to-peer file sharing with E2E encryption

## Architecture Summary

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri App                            │
├─────────────────────────────────────────────────────────┤
│  Frontend (React 18)          │  Backend (Rust)         │
│  ├── Components/              │  ├── core/              │
│  ├── hooks/                   │  │   ├── drive.rs       │
│  ├── stores/                  │  │   ├── file.rs        │
│  └── lib/                     │  │   └── identity.rs    │
│                               │  ├── network/           │
│                               │  │   ├── sync.rs        │
│                               │  │   └── gossip.rs      │
│                               │  ├── crypto/            │
│                               │  │   ├── encryption.rs  │
│                               │  │   └── keys.rs        │
│                               │  └── storage/           │
│                               │      └── db.rs          │
└─────────────────────────────────────────────────────────┘
```

## Key Technologies

| Layer | Technology | Purpose |
|-------|------------|---------|
| Framework | Tauri v2 | Cross-platform desktop |
| P2P | Iroh | QUIC networking, blobs, gossip |
| Encryption | ChaCha20-Poly1305 | File encryption |
| Key Exchange | X25519 | Per-user key wrapping |
| Hashing | BLAKE3 | Content addressing |
| Database | redb | Embedded key-value store |
| Frontend | React 18 | UI with concurrent features |

## Code Style Guidelines

### Rust

```rust
// Use Result with anyhow for error handling
pub async fn create_drive(name: String, path: PathBuf) -> anyhow::Result<SharedDrive> {
    // ...
}

// Prefer strong typing over primitives
pub struct DriveId([u8; 32]);  // Not: pub type DriveId = [u8; 32];

// Use derive macros liberally
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry { ... }

// Document public APIs
/// Creates a new shared drive from a local folder.
/// 
/// # Arguments
/// * `name` - Human-readable drive name
/// * `path` - Local filesystem path to share
pub fn new(name: String, path: PathBuf) -> Self { ... }
```

### TypeScript

```typescript
// Use strict TypeScript
interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modifiedAt: string;
}

// Use Tauri invoke for backend calls
const files = await invoke<FileEntry[]>('list_files', { driveId, path });

// Use React hooks for state
function useFiles(driveId: string, path: string) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  // ...
}
```

## Common Patterns

### Tauri Commands

```rust
#[tauri::command]
async fn command_name(
    param: String,
    state: State<'_, AppState>,
) -> Result<ReturnType, String> {
    state.service
        .do_something(param)
        .await
        .map_err(|e| e.to_string())
}
```

### Event Emission

```rust
// Backend to frontend
window.emit("drive-event", DriveEventPayload { 
    drive_id, 
    event 
})?;

// Frontend listening
listen<DriveEventPayload>('drive-event', (event) => {
    handleEvent(event.payload);
});
```

### Encryption Pattern

```rust
// Always use authenticated encryption
let cipher = ChaCha20Poly1305::new(&key);
let nonce = Nonce::from_slice(&random_bytes::<12>());
let ciphertext = cipher.encrypt(nonce, plaintext)?;
```

## Important Constraints

1. **Performance:** Target 120 FPS UI, <10ms LAN latency
2. **Security:** Zero-knowledge - owner controls all keys
3. **Offline-first:** App works without network
4. **Cross-platform:** Windows, macOS, Linux support

## File Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Rust modules | snake_case | `drive_manager.rs` |
| Rust structs | PascalCase | `SharedDrive` |
| React components | PascalCase | `FileList.tsx` |
| React hooks | camelCase | `useFiles.ts` |
| CSS modules | camelCase | `fileList.module.css` |

## Testing Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_drive() {
        let drive = create_drive("Test".into(), "/tmp/test".into())
            .await
            .unwrap();
        assert_eq!(drive.name, "Test");
    }
}
```

## Dependencies Reference

See [Cargo.reference.toml](./Cargo.reference.toml) for full dependency list.

## Related Documentation

- [Architecture](./architecture.md) - System design
- [Performance](./performance.md) - Optimization strategies
- [Security](./security.md) - Encryption and access control
- [API Reference](./api-reference.md) - Tauri commands
