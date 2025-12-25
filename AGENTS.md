# AGENTS.md

This file provides guidance to Agents when working with code in this repository.

## Project Overview

P2P Realtime Drive Sharing System - A self-hosted peer-to-peer file sharing desktop application with enterprise-grade security. Built with Tauri v2 (Rust backend + React frontend) using Iroh for P2P networking.

## Codebase Navigation tool (morph-mcp)

morph-mcp: warpgrep_codebase_search is a subagent that takes in a search string and tries to find relevant context. Best practice is to use it at the beginning of codebase explorations to fast track finding relevant files/lines. Do not use it to pin point keywords, but use it for broader semantic queries. "Find the XYZ flow", "How does XYZ work", "Where is XYZ handled?", "Where is <error message> coming from?"

## Build & Development Commands

```bash
# Install dependencies
bun install
cd src-tauri && cargo build

# Run development
bun tauri dev

# Run tests
cd src-tauri && cargo test

# Run a single test
cd src-tauri && cargo test test_name

# Lint
bun lint
cd src-tauri && cargo clippy

# Check Rust code (faster than full build)
cd src-tauri && cargo check

# Run benchmarks
cd src-tauri && cargo bench
```

## Architecture

### Module Structure (Rust Backend)

```
src-tauri/src/
├── main.rs             # Binary entry point
├── lib.rs              # App entry, Tauri setup, event forwarders
├── state.rs            # AppState with identity, network, sync engine
├── tray.rs             # System tray icon and menu
├── core/               # Core data structures & managers
│   ├── drive.rs        # SharedDrive, DriveInfo
│   ├── file.rs         # FileEntryDto
│   ├── identity.rs     # IdentityManager, Ed25519 keypair
│   ├── events.rs       # DriveEvent, DriveEventDto, EventBroadcaster
│   ├── watcher.rs      # FileWatcherManager (notify crate)
│   ├── locking.rs      # FileLock, LockManager, DriveLockManager
│   ├── conflict.rs     # FileConflict, ConflictManager
│   ├── presence.rs     # UserPresence, PresenceManager, ActivityEntry
│   └── channel.rs      # Async channel utilities
├── network/            # P2P networking
│   ├── endpoint.rs     # Iroh QUIC endpoint setup
│   ├── gossip.rs       # iroh-gossip event broadcasting
│   ├── docs.rs         # iroh-docs CRDT metadata sync
│   ├── sync.rs         # SyncEngine orchestration
│   └── transfer.rs     # FileTransferManager via iroh-blobs
├── crypto/             # Cryptography
│   ├── keys.rs         # Key generation/management
│   ├── encryption.rs   # DriveEncryption (ChaCha20-Poly1305)
│   ├── key_exchange.rs # X25519 key exchange, KeyRing
│   ├── access.rs       # AccessControlList, Permission, PathRule
│   └── invite.rs       # InviteToken, InviteBuilder, TokenTracker
├── storage/            # Persistence
│   └── db.rs           # redb database operations
└── commands/           # Tauri commands (frontend invoke handlers)
    ├── identity.rs     # get_identity, get_connection_status
    ├── drive.rs        # create_drive, delete_drive, list_drives, etc.
    ├── files.rs        # list_files
    ├── sync.rs         # start_sync, file transfers, watching
    ├── security.rs     # invite generation, permissions
    ├── locking.rs      # acquire_lock, release_lock, etc.
    ├── conflict.rs     # list_conflicts, resolve_conflict, etc.
    └── presence.rs     # presence tracking, activity feed
```

### Frontend Structure

```
src/
├── App.tsx                      # Main app with sidebar layout
├── main.tsx                     # Entry point
├── types.ts                     # Shared TypeScript types
├── components/
│   ├── DriveList.tsx            # Sidebar drive list
│   ├── FileBrowser.tsx          # File grid with icons, locking UI
│   ├── IdentityBadge.tsx        # Node ID display
│   ├── CreateDriveModal.tsx     # New drive dialog
│   ├── ShareDriveModal.tsx      # Invite/permissions tabs
│   ├── ConflictPanel.tsx        # Conflict resolution UI
│   ├── PresencePanel.tsx        # Online users & activity
│   ├── Titlebar.tsx             # Custom window titlebar
│   ├── InviteHandler.tsx        # Deep link invite processing
│   └── UpdateNotification.tsx   # Auto-update UI
└── hooks/
    ├── useDriveEvents.ts        # Subscribe to drive-event Tauri events
    ├── useFileTransfer.ts       # Upload/download progress
    ├── useLocking.ts            # File lock management
    ├── useConflicts.ts          # Conflict state & resolution
    ├── usePresence.ts           # Online users & activity feed
    ├── useDeepLink.ts           # Handle gix:// deep links
    └── useUpdater.ts            # App auto-update management
```

### Key Components

- **AppState**: Manages identity, network endpoint, database, drives, sync engine
- **SyncEngine**: Orchestrates iroh-docs (metadata), iroh-gossip (events), iroh-blobs (transfers)
- **EventBroadcaster**: Distributes DriveEvents to frontend via Tauri emit
- **FileWatcherManager**: Monitors local file changes with debouncing
- **LockManager/ConflictManager/PresenceManager**: Collaboration state managers

### Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Framework | Tauri v2 | Cross-platform desktop |
| P2P | Iroh | QUIC networking, blobs, gossip, docs |
| Encryption | ChaCha20-Poly1305 | File content E2E encryption |
| Key Exchange | X25519 | Per-user key wrapping |
| Hashing | BLAKE3 | Content addressing, integrity |
| Database | redb | Embedded key-value store |
| Frontend | React 18 | UI with concurrent features |
| Styling | SCSS | Component styles in `src/styles/` |

### Tauri Plugins

The app uses these Tauri v2 plugins:
- `tauri-plugin-dialog` - Native file/folder dialogs
- `tauri-plugin-fs` - File system access
- `tauri-plugin-shell` - Shell command execution
- `tauri-plugin-notification` - System notifications
- `tauri-plugin-autostart` - Launch on system startup
- `tauri-plugin-single-instance` - Prevent multiple instances
- `tauri-plugin-updater` - Auto-update support
- `tauri-plugin-deep-link` - Handle `gix://` URL scheme

## Code Conventions

### Rust
- Use `anyhow::Result` for application code, `thiserror` for library errors
- All async functions use `tokio` runtime
- Prefer `Bytes` and zero-copy patterns for file data
- Add `#[derive(Clone, Debug, Serialize, Deserialize)]` to data structs
- Use `#[tauri::command]` for frontend-callable functions

### TypeScript
- Strict TypeScript with no `any` types
- Import Tauri APIs from `@tauri-apps/api`
- Use React 18 concurrent features (useDeferredValue, useTransition)
- Invoke Tauri commands via `invoke<T>("command_name", { args })`

### Naming
- Rust: snake_case files, PascalCase types
- React: PascalCase components, camelCase hooks
- Use descriptive names: `SharedDrive` not `SD`

## Performance Constraints

- UI: 120 FPS (8.33ms per frame)
- LAN latency: <10ms
- Memory: <200MB base
- Use zero-copy buffers for file transfers

## Code Search Tips

- Find drive operations: Search for `SharedDrive` or `DriveManager`
- Find sync logic: Search for `SyncEngine` or `DriveEvent`
- Find encryption: Search for `DriveEncryption` or `ChaCha20`
- Find permissions: Search for `AccessControlList` or `Permission`
- Find Tauri commands: Search for `#[tauri::command]`
- Find file locking: Search for `LockManager` or `FileLock`
- Find conflicts: Search for `ConflictManager` or `FileConflict`
- Find presence: Search for `PresenceManager` or `UserPresence`

## Current Tauri Commands

```rust
// Identity
get_identity() -> IdentityInfo
get_connection_status() -> ConnectionStatus

// Drives
create_drive(name, path) -> DriveInfo
delete_drive(id)
rename_drive(id, new_name) -> DriveInfo
list_drives() -> Vec<DriveInfo>
get_drive(id) -> DriveInfo

// Files
list_files(drive_id, path) -> Vec<FileEntryDto>

// Sync & Transfers
start_sync(drive_id)
stop_sync(drive_id)
get_sync_status(drive_id) -> SyncStatus
subscribe_drive_events(drive_id)
start_watching(drive_id), stop_watching(drive_id), is_watching(drive_id)
upload_file(drive_id, path), download_file(drive_id, path, hash)
list_transfers(drive_id), get_transfer(id), cancel_transfer(id)

// Security & Invites
generate_invite(drive_id, permission, expires_in) -> InviteToken
verify_invite(token) -> InviteInfo
list_permissions(drive_id) -> Vec<PermissionEntry>
grant_permission(drive_id, user_id, permission)
revoke_permission(drive_id, user_id)
check_permission(drive_id, user_id, operation) -> bool

// File Locking
acquire_lock(drive_id, path, lock_type) -> FileLock
release_lock(drive_id, path)
get_lock_status(drive_id, path) -> Option<FileLock>
list_locks(drive_id) -> Vec<FileLock>
extend_lock(drive_id, path, duration)
force_release_lock(drive_id, path) // admin only

// Conflict Resolution
list_conflicts(drive_id) -> Vec<FileConflict>
get_conflict(drive_id, conflict_id) -> FileConflict
resolve_conflict(drive_id, conflict_id, strategy)
get_conflict_count(drive_id) -> usize
dismiss_conflict(drive_id, conflict_id)

// Presence & Activity
get_online_users(drive_id) -> Vec<UserPresence>
get_online_count(drive_id) -> usize
get_recent_activity(drive_id, limit) -> Vec<ActivityEntry>
join_drive_presence(drive_id)
leave_drive_presence(drive_id)
presence_heartbeat(drive_id)
```

## Tauri Events (Frontend Subscriptions)

```typescript
// Listen for drive events in React
import { listen } from '@tauri-apps/api/event';

listen<DriveEventDto>('drive-event', (event) => {
  // event.payload contains: drive_id, event_type, path, timestamp, etc.
});
```

## Documentation

Detailed documentation is in `docs/p2p-drive/`:
- `architecture.md` - System design and data models
- `security.md` - E2E encryption and access control
- `performance.md` - 120 FPS optimization strategies
- `api-reference.md` - Tauri commands reference
- `IMPLEMENTATION_PLAN.md` - Phase breakdown with progress
