# Gix - P2P Drive Share

Peer-to-peer desktop app for realtime folder sharing.

## Usage

```bash
# Install dependencies
bun install

# Run development mode
bun tauri dev
```

## Minimal Technical Info

- Tauri v2 app with React frontend and Rust backend
- P2P networking via Iroh (QUIC, blobs, gossip, docs)
- Embedded storage: redb
- Crypto: Ed25519, BLAKE3

## Docs

Project docs live in `docs/p2p-drive/`.

## License

MIT
