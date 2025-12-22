# AGENTS.md

This file provides guidance to Agents when working with code in this repository.

# Current time

> 2025-12-22

## Project Overview

P2P Realtime Drive Sharing System - A self-hosted peer-to-peer file sharing desktop application with enterprise-grade security. Built with Tauri v2 (Rust backend + React frontend) using Iroh for P2P networking.

## Build & Development Commands

```bash
# Install dependencies
pnpm install
cd src-tauri && cargo build

# Run development
pnpm tauri dev

# Run tests
cd src-tauri && cargo test

# Lint
pnpm lint
cd src-tauri && cargo clippy
```

## Architecture

### Module Structure (Rust Backend)

```
src-tauri/src/
├── core/           # Core data structures
│   ├── drive.rs    # SharedDrive, DriveId
│   ├── file.rs     # FileEntry, sync operations
│   └── identity.rs # NodeId, keypair management
├── network/        # P2P networking
│   ├── endpoint.rs # Iroh endpoint setup
│   ├── sync.rs     # SyncEngine implementation
│   └── gossip.rs   # DriveEvent broadcasting
├── crypto/         # Cryptography
│   ├── encryption.rs # DriveEncryption
│   └── keys.rs     # Key wrapping/unwrapping
├── storage/        # Persistence
│   └── db.rs       # redb database ops
└── commands/       # Tauri commands
    ├── drive.rs    # create_drive, list_drives
    ├── file.rs     # list_files, read_file
    └── invite.rs   # create_invite, join_drive
```

### Key Components

- **SharedDrive**: Folder shared by owner that peers can mount and access
- **SyncEngine**: Real-time sync using iroh-gossip (events) + iroh-docs (metadata)
- **DriveEncryption**: E2E encryption with ChaCha20-Poly1305, per-user X25519 key wrapping
- **AccessControlList**: Permission levels (Read < Write < Manage < Admin)

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
| State | Zustand | Global state management |

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
- Implement virtual scrolling for large lists

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

## Documentation

Detailed documentation is in `docs/p2p-drive/`:
- `architecture.md` - System design and data models
- `security.md` - E2E encryption and access control
- `performance.md` - 120 FPS optimization strategies
- `api-reference.md` - Tauri commands reference
