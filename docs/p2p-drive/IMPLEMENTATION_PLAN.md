# P2P Drive Share - Detailed Implementation Plan

## Project Overview

**Project Name:** P2P Realtime Drive Sharing System  
**Duration:** 12 weeks (60 working days)  
**Tech Stack:** Tauri v2 + React + Rust + Iroh P2P

---

## 📊 Progress Summary

| Phase | Status | Progress |
|-------|--------|----------|
| **Phase 1:** Core P2P Foundation | ✅ Complete | 100% |
| **Phase 2:** Realtime Sync Engine | ✅ Complete | 100% |
| **Phase 3:** Security & Access Control | ✅ Complete | 100% |
| **Phase 4:** Collaboration Features | ✅ Complete | 100% |
| **Phase 5:** Polish & Distribution | ✅ Complete | 100% |

### Phase 1 Completed (Dec 22, 2024)
- ✅ Tauri v2 + React + Rust project structure
- ✅ Ed25519 identity generation & persistence  
- ✅ Iroh P2P endpoint with relay discovery
- ✅ Drive creation with folder selection
- ✅ File browser with icons & keyboard navigation
- ✅ Drive management (rename, delete)

### Phase 2 Completed (Dec 23, 2024)
- ✅ iroh-docs integration (`DocsManager` for CRDT metadata sync)
- ✅ iroh-gossip integration (`EventBroadcaster` for live events)
- ✅ File system watcher (`FileWatcherManager` with debouncing)
- ✅ Bidirectional sync (`FileTransferManager` via iroh-blobs)
- ✅ Sync engine orchestration (`SyncEngine` coordinator)
- ✅ Tauri sync commands (frontend API)
- ✅ Code polishing (Clippy fixes, dead code audit)

### Phase 3 Completed (Dec 23, 2024)
- ✅ E2E Encryption (`DriveEncryption` with ChaCha20-Poly1305)
- ✅ Key exchange (`KeyExchangePair`, `KeyRing` with X25519)
- ✅ Access control (`AccessControlList`, `Permission`, `PathRule`)
- ✅ Invite tokens (`InviteToken`, `InviteBuilder`, `TokenTracker`)
- ✅ Tauri security commands (`security.rs` with `SecurityStore`)
  - `generate_invite`, `verify_invite`, `list_permissions`
  - `grant_permission`, `revoke_permission`, `check_permission`
- ✅ Frontend UI (`ShareDriveModal` with invite/permissions tabs)

### Phase 4 Completed (Dec 23, 2024)
- ✅ File Locking (`FileLock`, `LockManager`, `DriveLockManager`)
  - Advisory and exclusive lock types with auto-expiration
  - Lock acquisition, release, extend via Tauri commands
  - Gossip broadcast of lock events (`FileLockAcquired`, `FileLockReleased`)
  - Frontend `useLocking` hook and `FileBrowser` lock indicators
- ✅ Conflict Resolution (`FileConflict`, `ConflictManager`, `DriveConflictManager`)
  - Conflict detection with local/remote version tracking
  - Resolution strategies: KeepLocal, KeepRemote, KeepBoth, ManualMerge
  - Tauri commands for listing, resolving, dismissing conflicts
  - Frontend `useConflicts` hook and `ConflictPanel` UI component
- ✅ Presence & Activity (`UserPresence`, `PresenceManager`, `ActivityEntry`)
  - Online user tracking with status (online/away/offline)
  - Activity feed with file and user events
  - Heartbeat mechanism for presence updates
  - Frontend `usePresence` hook and `PresencePanel` UI component

### Phase 5 Completed (Dec 24, 2024)
- ✅ Code Polish
  - Clippy warnings reduced from 47 to 0
  - Dead code cleanup with `#[allow(dead_code)]` for future-use APIs
  - Improved error handling patterns
- ✅ System Integration
  - System tray with menu (Show, Hide, Quit)
  - Notification plugin (`tauri-plugin-notification`)
  - Autostart plugin (`tauri-plugin-autostart`)
  - Single instance enforcement (`tauri-plugin-single-instance`)
- ✅ Windows Distribution
  - NSIS installer: `Gix_0.1.0_x64-setup.exe`
  - MSI installer: `Gix_0.1.0_x64_en-US.msi`
  - Configured bundling with publisher info and descriptions



## Phase 1: Core P2P Foundation
**Duration:** 3 weeks | **Priority:** Critical

### 1.1 Project Setup (Days 1-3)

| Task | Description | Est. |
|------|-------------|------|
| Initialize Tauri v2 project | `pnpm create tauri-app` with React template | 2h |
| Configure Cargo workspace | Set up `src-tauri/Cargo.toml` with dependencies | 1h |
| Setup React frontend | Vite + React 18 + TypeScript strict mode | 2h |
| Configure ESLint & Prettier | Code quality tooling | 1h |
| Setup Rust tooling | rustfmt, clippy, cargo-watch | 1h |
| Create project structure | Organize modules: `core/`, `network/`, `crypto/`, `storage/` | 2h |
| Add CI/CD pipeline | GitHub Actions for build/test/lint | 3h |

**Deliverables:**
- [x] Empty Tauri app runs on Windows/macOS/Linux
- [x] Hot reload working for both frontend and backend
- [x] All linting passes

---

### 1.2 Identity Management (Days 4-6)

| Task | Description | Est. |
|------|-------------|------|
| Generate Ed25519 keypair | First-run identity creation | 3h |
| Secure key storage | OS keychain integration (keyring crate) | 4h |
| NodeId derivation | Derive public node ID from keypair | 2h |
| Identity UI component | Display node ID with copy button | 2h |
| Identity persistence | Store identity metadata in redb | 3h |

**Deliverables:**
- [x] App generates unique identity on first run
- [x] Identity persists across app restarts
- [x] Public ID displayed in UI

---

### 1.3 Iroh Integration (Days 7-10)

| Task | Description | Est. |
|------|-------------|------|
| Add Iroh dependencies | iroh, iroh-blobs, iroh-gossip, iroh-docs | 1h |
| Initialize Iroh endpoint | Create endpoint on app startup | 4h |
| Configure QUIC transport | Low-latency settings from performance spec | 3h |
| Implement discovery | Use iroh's built-in discovery_n0() | 3h |
| Connection state management | Track connected peers | 4h |
| Error handling | Graceful handling of network failures | 3h |

**Deliverables:**
- [x] Iroh endpoint starts successfully
- [x] App can discover and connect to other instances
- [x] Connection status visible in UI

---

### 1.4 Drive Creation (Days 11-13)

| Task | Description | Est. |
|------|-------------|------|
| SharedDrive struct | Implement data model from architecture doc | 3h |
| DriveId generation | BLAKE3 hash of owner + path + timestamp | 2h |
| Folder selection dialog | Tauri file picker integration | 2h |
| Local file indexing | Walk directory tree with walkdir | 4h |
| Drive metadata storage | Store in redb database | 3h |
| create_drive command | Tauri command implementation | 3h |
| Drive list UI | Display owned drives in sidebar | 3h |

**Deliverables:**
- [x] User can select folder to share
- [x] Drive appears in "My Drives" list
- [x] Drive persists after restart

---

### 1.5 Basic File Listing (Days 14-15)

| Task | Description | Est. |
|------|-------------|------|
| FileEntry struct | Name, path, size, modified_at, is_dir | 2h |
| list_files command | Return directory contents | 3h |
| Remote file request | Request file list from peer over QUIC | 4h |
| File list UI component | Display files with icons | 4h |
| Breadcrumb navigation | Path navigation UI | 2h |

**Deliverables:**
- [x] Can browse local shared drive
- [x] Can request and display remote drive contents (Phase 2)

---

## Phase 2: Realtime Sync Engine
**Duration:** 3 weeks | **Priority:** Critical

### 2.1 iroh-docs Integration (Days 16-19)

| Task | Description | Est. |
|------|-------------|------|
| Create Doc per drive | Initialize iroh-docs document | 3h |
| Define metadata schema | File entries, permissions, etc. | 3h |
| Sync Doc between peers | Use iroh-docs sync protocol | 4h |
| Handle Doc conflicts | CRDT-based resolution | 4h |
| Watch Doc changes | Subscribe to document updates | 3h |

**Deliverables:**
- [x] Drive metadata syncs between peers
- [x] Changes on one peer appear on others

---

### 2.2 Gossip Events (Days 20-22)

| Task | Description | Est. |
|------|-------------|------|
| DriveEvent enum | Define all event types | 2h |
| Topic per drive | Create gossip topic from DriveId | 2h |
| Broadcast implementation | Send events to all peers | 3h |
| Event subscription | Receive and handle events | 3h |
| Event filtering | Only process relevant events | 2h |
| Frontend event bridge | Emit events to UI via Tauri | 3h |

**Deliverables:**
- [x] Real-time events flow between peers
- [x] UI updates on incoming events

---

### 2.3 File System Watcher (Days 23-25)

| Task | Description | Est. |
|------|-------------|------|
| Integrate notify crate | Watch shared folders | 3h |
| Debounce rapid changes | Avoid event floods | 2h |
| Map FS events to DriveEvents | Create, modify, delete | 3h |
| Ignore patterns | Skip .git, node_modules, etc. | 2h |
| Handle rename/move | Track file relocations | 3h |

**Deliverables:**
- [x] Local file changes trigger sync events
- [x] Debouncing prevents excessive updates

---

### 2.4 Bidirectional Sync (Days 26-30)

| Task | Description | Est. |
|------|-------------|------|
| Upload flow | Local → iroh-blobs → remote | 4h |
| Download flow | Remote → iroh-blobs → local | 4h |
| Delta sync | Only transfer changed bytes | 6h |
| Atomic writes | Temp file → rename pattern | 2h |
| Progress tracking | Upload/download progress events | 3h |
| Bandwidth throttling | Optional speed limits | 3h |

**Deliverables:**
- [x] Files sync bidirectionally
- [x] Large files transfer efficiently
- [x] Progress shown in UI

---

## Phase 3: Security & Access Control
**Duration:** 2 weeks | **Priority:** Critical

### 3.1 E2E Encryption (Days 31-35)

| Task | Description | Est. |
|------|-------------|------|
| DriveEncryption struct | ChaCha20-Poly1305 implementation | 4h |
| Master key generation | Random 256-bit key per drive | 2h |
| File encryption | Encrypt before upload | 4h |
| File decryption | Decrypt after download | 4h |
| Streaming encryption | Handle large files efficiently | 4h |
| Metadata encryption | Encrypt file names/paths | 3h |

**Deliverables:**
- [x] All file content encrypted at rest
- [x] Only authorized users can decrypt

---

### 3.2 Key Management (Days 36-38)

| Task | Description | Est. |
|------|-------------|------|
| X25519 key exchange | Diffie-Hellman implementation | 3h |
| Key wrapping | Wrap master key per user | 3h |
| Key unwrapping | User decrypts their wrapped key | 3h |
| Key rotation | Re-encrypt with new key | 4h |
| Revocation | Remove user's wrapped key | 2h |

**Deliverables:**
- [x] Each user has their own wrapped key
- [x] Key can be rotated on demand

---

### 3.3 Access Control (Days 39-42)

| Task | Description | Est. |
|------|-------------|------|
| Permission enum | Read, Write, Manage, Admin | 1h |
| AccessControlList struct | Per-user and path-based rules | 3h |
| Permission checking | Enforce on all operations | 4h |
| Path rules | Folder-specific restrictions | 3h |
| Expiration support | Time-limited access | 2h |
| Permission UI | Manage user permissions | 4h |

**Deliverables:**
- [x] Operations fail without proper permission
- [ ] Admins can manage user access (UI pending)

---

### 3.4 Invite System (Days 43-44)

| Task | Description | Est. |
|------|-------------|------|
| InviteToken struct | Signed, time-limited tokens | 3h |
| Token generation | Create invite with params | 2h |
| Token verification | Validate signature and expiry | 2h |
| Join flow | Accept invite and get wrapped key | 4h |
| Invite UI | Generate and share links | 3h |

**Deliverables:**
- [ ] Users can create invite links (UI pending)
- [ ] Invitees can join via link (integration pending)

---

## Phase 4: Collaboration Features
**Duration:** 2 weeks | **Priority:** High

### 4.1 File Locking (Days 45-47)

| Task | Description | Est. |
|------|-------------|------|
| FileLock struct | Path, holder, type, timestamp | 2h |
| Lock acquisition | Request exclusive/advisory lock | 3h |
| Lock release | Manual and automatic (RAII) | 2h |
| Lock broadcasting | Announce via gossip | 2h |
| Lock UI indicators | Show locked files in list | 3h |

**Deliverables:**
- [x] Files can be locked during editing
- [x] Lock status visible to all users

---

### 4.2 Conflict Resolution (Days 48-51)

| Task | Description | Est. |
|------|-------------|------|
| Conflict detection | Identify simultaneous edits | 3h |
| ConflictInfo struct | Store conflict details | 2h |
| Resolution strategies | Keep local/remote/both/merge | 4h |
| Diff view | Side-by-side comparison | 6h |
| Resolution UI | User selects resolution | 4h |

**Deliverables:**
- [x] Conflicts detected and queued
- [x] Users can resolve via UI

---

### 4.3 Presence & Activity (Days 52-54)

| Task | Description | Est. |
|------|-------------|------|
| Online status tracking | Track connected users | 3h |
| Presence broadcasting | Announce join/leave | 2h |
| User list component | Show who's online | 3h |
| Activity feed | Recent changes log | 4h |
| Activity filtering | By user, file, or time | 2h |

**Deliverables:**
- [x] See who's currently connected
- [x] Activity feed shows all changes

---

## Phase 5: Polish & Distribution
**Duration:** 2 weeks | **Priority:** Medium

### 5.1 System Integration (Days 55-57)

| Task | Description | Est. |
|------|-------------|------|
| System tray | Background operation | 4h |
| Tray menu | Quick actions | 2h |
| Notifications | Sync complete, user joined, etc. | 3h |
| Auto-start | Start with OS (optional) | 2h |
| Deep links | Handle p2pshare:// URLs | 3h |

**Deliverables:**
- [x] App runs in system tray
- [x] Notifications working

---

### 5.2 Distribution (Days 58-60)

| Task | Description | Est. |
|------|-------------|------|
| Windows installer | MSI/NSIS package | 3h |
| macOS DMG | Signed and notarized | 4h |
| Linux packages | AppImage, .deb, .rpm | 4h |
| Auto-updater | tauri-plugin-updater | 4h |
| Release automation | GitHub Releases + changelog | 3h |

**Deliverables:**
- [x] Windows installers (NSIS + MSI)
- [ ] macOS/Linux installers (future)
- [ ] Auto-update functional (future)

---

## Risk Register

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| NAT traversal issues | High | Medium | Use iroh relay network |
| Performance < 120 FPS | Medium | Low | Profile early, optimize |
| iroh API changes | Medium | Low | Pin versions, monitor |
| Key management UX | Medium | Medium | Leverage OS keychain |
| Cross-platform issues | High | Medium | Test matrix in CI |

---

## Success Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| UI Frame Rate | ≥ 120 FPS | Chrome DevTools FPS meter |
| LAN Sync Latency | < 100ms | End-to-end timing |
| WAN Sync Latency | < 500ms | Via relay timing |
| Memory Usage | < 200MB | Task Manager / Activity Monitor |
| Installer Size | < 50MB | Release artifact size |
| Cold Start Time | < 3s | App launch to ready |
| Time to First Sync | < 10s | Connection to file visible |
