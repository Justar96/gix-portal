# Project Structure

## Root Layout
```
├── src/                 # React frontend
├── src-tauri/           # Rust backend (Tauri)
├── docs/p2p-drive/      # Architecture & design docs
└── .kiro/steering/      # AI steering rules
```

## Frontend (`src/`)
```
src/
├── App.tsx              # Main app with sidebar layout
├── main.tsx             # Entry point
├── types.ts             # Shared TypeScript types
├── components/          # React components (PascalCase)
│   ├── DriveList.tsx
│   ├── FileBrowser.tsx
│   ├── DriveWorkspace.tsx
│   ├── *Modal.tsx       # Dialog components
│   └── *Panel.tsx       # Side panels
├── hooks/               # Custom hooks (camelCase, use* prefix)
│   ├── useDriveEvents.ts
│   ├── useFileTransfer.ts
│   ├── useLocking.ts
│   └── usePresence.ts
├── styles/              # SCSS organized by type
│   ├── abstracts/       # Variables, mixins
│   ├── base/            # Reset, typography
│   ├── components/      # Component styles (_component-name.scss)
│   └── main.scss        # Entry point
└── test/                # Test setup and mocks
```

## Backend (`src-tauri/src/`)
```
src-tauri/src/
├── main.rs              # Binary entry
├── lib.rs               # Tauri setup, event forwarders
├── state.rs             # AppState (identity, network, sync)
├── tray.rs              # System tray
├── commands/            # Tauri commands (#[tauri::command])
│   ├── drive.rs         # CRUD operations
│   ├── files.rs         # File listing
│   ├── sync.rs          # Sync & transfers
│   ├── security.rs      # Invites, permissions
│   ├── locking.rs       # File locks
│   ├── conflict.rs      # Conflict resolution
│   └── presence.rs      # Online users
├── core/                # Domain logic & managers
│   ├── drive.rs         # SharedDrive, DriveInfo
│   ├── events.rs        # DriveEvent, EventBroadcaster
│   ├── locking.rs       # LockManager
│   ├── conflict.rs      # ConflictManager
│   └── presence.rs      # PresenceManager
├── network/             # P2P layer
│   ├── endpoint.rs      # Iroh QUIC setup
│   ├── gossip.rs        # Event broadcasting
│   ├── docs.rs          # CRDT metadata sync
│   ├── sync.rs          # SyncEngine orchestration
│   └── transfer.rs      # FileTransferManager
├── crypto/              # Security
│   ├── encryption.rs    # ChaCha20-Poly1305
│   ├── keys.rs          # Key management
│   ├── access.rs        # ACL, permissions
│   └── invite.rs        # Invite tokens
└── storage/
    └── db.rs            # redb operations
```

## Key Patterns
- Commands in `commands/` are thin wrappers calling `core/` or `network/`
- Managers (LockManager, ConflictManager) handle domain state
- SyncEngine orchestrates docs + gossip + blobs
- Frontend invokes commands via `invoke<T>("command_name", { args })`
- Events flow backend → frontend via Tauri emit (`drive-event`)
