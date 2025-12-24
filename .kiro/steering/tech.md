# Technology Stack

## Framework
- Tauri v2 - Cross-platform desktop app (Rust backend + web frontend)

## Backend (Rust)
- Runtime: Tokio async
- P2P: Iroh (QUIC networking, blobs, gossip, docs CRDT)
- Encryption: ChaCha20-Poly1305 (content), X25519 (key exchange), BLAKE3 (hashing)
- Database: redb (embedded key-value store)
- File watching: notify crate with debouncing
- Error handling: anyhow (app code), thiserror (library errors)

## Frontend (TypeScript/React)
- React 18 with concurrent features (useDeferredValue, useTransition)
- Styling: SCSS with component-based organization
- Icons: lucide-react
- Virtualization: @tanstack/react-virtual
- Testing: Vitest + React Testing Library

## Tauri Plugins
- dialog, fs, shell, notification, autostart
- single-instance, updater, deep-link

## Build Commands

```bash
# Install dependencies
pnpm install
cd src-tauri && cargo build

# Development
pnpm tauri dev

# Frontend tests
pnpm test              # single run
pnpm test:watch        # watch mode
pnpm test:coverage     # with coverage

# Rust tests
cd src-tauri && cargo test
cd src-tauri && cargo test test_name  # single test

# Linting
pnpm lint
cd src-tauri && cargo clippy

# Fast Rust check (no build)
cd src-tauri && cargo check

# Benchmarks
cd src-tauri && cargo bench
```

## Performance Targets
- UI: 120 FPS (8.33ms per frame)
- LAN latency: <10ms
- Memory: <200MB base
- Use zero-copy buffers for file transfers
