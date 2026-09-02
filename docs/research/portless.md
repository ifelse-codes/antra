# Portless Research

## What is Portless

Portless (vercel-labs/portless) is a TypeScript/Node.js developer tool that replaces unpredictable localhost ports with stable `.localhost` URLs. 11,400+ GitHub stars.

## Architecture

Two-process model:
1. **Proxy Daemon** — Long-lived HTTPS server, handles TLS and routing
2. **App Process** — Your dev server, bound to an auto-assigned port

Route storage: `~/.portless/routes.json` (read on every request)

## What Antra Should LEARN

### 1. Two-Process Model
Daemon owns infrastructure, CLI is ephemeral. Separation of concerns.

### 2. Auto-Start Pattern
Proxy starts automatically when first app registers. Zero-config UX.

### 3. Per-Hostname SNI Certificates
Wildcard certs are invalid for `.localhost` (TLS spec). Generate per-hostname certs on SNI callback.

### 4. Loop Detection
`X-Portless-Hops` header prevents infinite proxy loops. Max 5 hops.

### 5. `doctor` Command
Comprehensive diagnostics essential for debugging complex proxy setups.

### 6. /etc/hosts Auto-Sync
Safari fallback for `.localhost` domains. Pragmatic cross-browser support.

### 7. Framework-Aware Injection
Auto-detect frameworks and inject `--port`, `--host` flags. Makes it work out-of-box.

## What Antra Should AVOID

### 1. Don't Shell Out to `openssl`
Portless shells out to system openssl for all cert operations.
- Runtime dependency (openssl must be installed)
- Slow (subprocess spawning)
- Fragile (different openssl versions)

**Antra**: Use `rcgen` (Rust-native, no external deps)

### 2. Don't Read routes.json Per Request
Portless reads the route file from disk on every HTTP request.
- Filesystem I/O per request = bottleneck under concurrency

**Antra**: In-memory `RwLock<HashMap>` with atomic updates

### 3. Don't Use Fixed Port Range
Portless uses 4000-4999 (1000 ports max).

**Antra**: Use OS ephemeral ports (port 0)

### 4. Don't Require Node.js
TypeScript original requires Node.js 24+.

**Antra**: Single Rust binary, no runtime dependencies

### 5. Don't Make TLS Optional
Rust port (portless-rs) has no TLS at all. TypeScript made it opt-in initially.

**Antra**: HTTPS first-class from Phase 4

### 6. Don't Use `lsof` for Process Discovery
Rust port uses `lsof` (Unix-specific, slow).

**Antra**: Use OS-native APIs or `TcpListener` bind attempts

### 7. Don't Forget WebSocket/HMR
Any proxy without WebSocket support is useless for modern frontend dev.

**Antra**: WebSocket from Phase 3, tested with Vite

### 8. Don't Ignore Windows
Rust port explicitly doesn't support Windows.

**Antra**: Platform abstraction from the start, Windows support in Phase 10

## Rust Port (portless-rs) Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1 | Async runtime |
| hyper | 1 | HTTP proxy |
| hyper-util | 0.1 | Server utilities |
| clap | 4 | CLI |
| serde/serde_json | 1 | Route serialization |
| dirs | 5 | Config dirs |
| nix | 0.29 | Unix APIs |
| colored | 2 | Terminal output |

Binary size: ~1MB static binary

## Key Takeaway

Portless proves the concept works. Antra's value is doing it in Rust (single binary, no Node.js dependency) with first-class HTTPS and cross-platform support.

## Sources
- vercel-labs/portless: https://github.com/vercel-labs/portless
- portless-rs/portless: https://github.com/portless-rs/portless
- DeepWiki: https://deepwiki.com/vercel-labs/portless
- portless.sh: https://portless.sh
