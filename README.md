# Gix - P2P Drive Share

A peer-to-peer desktop application for **realtime folder sharing** built with Tauri v2, React, and Rust.

## Features

### ✅ Implemented (Phase 1)
- **Identity Management** - Ed25519 keypair per device
- **P2P Networking** - Iroh QUIC endpoint with relay discovery
- **Drive Management** - Create, rename, delete shared drives
- **File Browser** - Navigate files with icons & keyboard shortcuts
- **Connection Status** - Real-time P2P connectivity indicator

### 🔜 Coming Soon (Phase 2)
- **Drive Sharing** - Invite links to share with others
- **Real-time Sync** - File changes sync across peers
- **Live Events** - See collaborators' activity

## Quick Start

```bash
# Install dependencies
npm install

# Run development mode
npm run tauri dev
```

## Tech Stack

- **Frontend:** React 18 + TypeScript + Vite
- **Backend:** Rust + Tauri v2
- **P2P:** Iroh (QUIC, blobs, gossip, docs)
- **Storage:** redb embedded database
- **Crypto:** Ed25519, BLAKE3

## Project Structure

```
gix-portal/
├── src/                    # React frontend
│   ├── components/         # UI components
│   └── types.ts           # Shared TypeScript types
├── src-tauri/             # Rust backend
│   └── src/
│       ├── commands/      # Tauri commands
│       ├── core/          # Business logic
│       ├── network/       # P2P networking
│       ├── crypto/        # Cryptography
│       └── storage/       # Database
└── docs/p2p-drive/        # Project documentation
```

## Documentation

See [docs/p2p-drive/](./docs/p2p-drive/) for:
- [Architecture](./docs/p2p-drive/architecture.md)
- [Implementation Plan](./docs/p2p-drive/IMPLEMENTATION_PLAN.md)
- [API Reference](./docs/p2p-drive/api-reference.md)
- [Security](./docs/p2p-drive/security.md)

## License

MIT
