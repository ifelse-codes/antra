# AGENT.md — Antra Development Driver

> **READ THIS FILE FIRST.** This is the single source of truth for building Antra.
> Everything you need — architecture, phases, rules, status, commands — is here.

---

## What is Antra

Antra is a native Rust developer CLI that maps stable domain names to local development servers.

```bash
antra run --domain myapp.localhost -- pnpm dev
```

Result:
```
ANTRA

✓ Proxy ready (port 443)
✓ HTTPS ready
✓ Route registered

  https://myapp.localhost
  → 127.0.0.1:5173
```

The user opens `https://myapp.localhost` and their app loads. No ports to remember.

---

## Project Status

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 0 | Scaffolding & Docs | ✅ DONE | 57 files, CLI skeleton, all docs |
| 1 | Minimal HTTP Reverse Proxy | ✅ DONE | HTTP proxy, route lookup, X-Forwarded-*, 502/503 errors |
| 2 | CLI Process Runner | ✅ DONE | `antra run` spawns child, registers route, cleans up on exit |
| 3 | WebSocket / HMR | ✅ DONE | Raw TCP tunnel, upgrade detection, loop detection |
| 4 | HTTPS / TLS | ✅ DONE | CA generation, SNI cert cache, TLS termination, HTTP→HTTPS redirect |
| 5 | Domain Resolution | ✅ DONE | .localhost no-op, .test/custom via hosts file, domain validation |
| 6 | Root CA Trust | ✅ DONE | os-truststore, install/status/remove, user prompts |
| 7 | Daemon + IPC | ✅ DONE | Unix socket IPC, JSON protocol, auto-start, idle shutdown |
| 8 | Route Management & DX | ✅ DONE | list, open, doctor, clean, alias commands |
| 9 | Configuration | ✅ DONE | antra.toml parsing, `antra dev` command, CLI flag overrides |
| 10 | Cross-Platform Hardening | ✅ DONE | Windows fixes, platform abstractions, CI/CD, release workflow |

**Current state:** All phases (0-10) complete. Antra is fully built with cross-platform support, CI/CD, and release workflow.

### Landing Page

- **Location:** `landing/index.html`
- **Deployed to:** Cloudflare Pages → `https://antra.iifelse.com`
- **Project name:** `antra-landing`
- **Design language:** Mudra (dark, surgical, violet accent)
- **To update:** `wrangler pages deploy . --project-name antra-landing`

---

## Critical Rules — DO NOT DEVIATE

### Naming
- Product name: **Antra** (never Antara)
- Binary: `antra`
- Config dir: `~/.config/antra/`
- Config file: `antra.toml`

### Architecture
- Language: Rust only. No Node.js, Python, or other runtimes.
- Async runtime: Tokio
- HTTP: hyper 1.x
- TLS: rustls + rcgen (Rust-native, no openssl)
- WebSocket: `copy_bidirectional` (raw tunnel, not frame-level)
- Route storage: In-memory `RwLock<HashMap>` (never read disk per request)
- CLI: Clap 4 derive

### Safety
- Never silently modify `/etc/hosts`
- Never silently install system certificates
- Never kill unrelated processes
- Never expose private keys
- Never log credentials or cookies
- Always prompt before system changes

### Scope
- Read `docs/mvp.md` for in-scope / out-of-scope items
- Each phase in `PLAN.md` has explicit **Exclusions (DO NOT BUILD)** — follow them
- Do not add features not in the current phase

### Process
- Build compiles with zero errors and zero warnings before moving to next phase
- Test manually after each phase
- Update this file's status table when a phase completes
- Do not accumulate untested changes

---

## Key Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Default TLD | `.localhost` | Browser-native, secure context, no hosts file |
| HTTPS | First-class from Phase 4 | Required for .test, custom domains |
| CA generation | `rcgen` | Rust-native, no openssl dependency |
| TLS | `rustls` + `tokio-rustls` | Memory safe, fast |
| WebSocket | `copy_bidirectional` | Transparent, minimal overhead, HMR works |
| Route storage | `RwLock<HashMap>` | Fast, no disk I/O per request |
| IPC | Unix socket / Named pipe | Platform-native |
| Daemon | Background, auto-start | Zero-config UX |

---

## How to Run

```bash
# Build
cargo build

# Run CLI
cargo run -- --help
cargo run -- run --domain myapp.localhost -- pnpm dev
cargo run -- list
cargo run -- doctor
cargo run -- proxy start
cargo run -- trust
```

---

## Phase 1 — Minimal HTTP Reverse Proxy ✅ DONE

### Verified
- HTTP proxy forwards requests to upstream by Host header
- 502 Bad Gateway for unknown domains
- 503 Service Unavailable for dead upstreams
- X-Forwarded-For, X-Forwarded-Proto, X-Forwarded-Host headers set correctly
- Host header rewritten to upstream address
- `antra proxy start --route domain:port` works

---

## Phase 2 — CLI Process Runner ✅ DONE

### Verified
- `antra run --domain X --port Y -- <cmd>` spawns child and registers route
- Proxy forwards requests to child process (200 OK)
- Route removed when child exits
- No orphan processes
- Signal forwarding via nix (SIGTERM to process group)

---

## Phase 3 — WebSocket / HMR Support ✅ DONE

### Verified
- WebSocket upgrade detection (Upgrade: websocket + Connection: upgrade)
- Raw TCP tunnel to upstream (hyper HTTP client doesn't support upgrades)
- `hyper::upgrade::on()` captures client upgrade connection
- `serve_connection_with_upgrades()` enables server-side upgrades
- `tokio::io::copy_bidirectional()` tunnels data between client and upstream
- Loop detection via `X-Antra-Hops` header (max 5 hops)
- Forwarded headers (X-Forwarded-For, X-Forwarded-Host, X-Forwarded-Proto) on WS requests
- `cargo build` with zero warnings

### Implementation Notes
- Used raw TCP for upstream connection (hyper HTTP client doesn't support upgrades)
- Client-side key forwarded to upstream for compatibility
- Server uses `auto::Builder::serve_connection_with_upgrades()` instead of `.with_upgrades()`

### Goal
WebSocket connections (including Vite HMR) tunnel through the proxy transparently.

### What to Build

1. **WebSocket detection** (`src/proxy/http.rs`):
   - Check for `Upgrade: websocket` + `Connection: upgrade` headers
   - If detected, delegate to WebSocket handler instead of HTTP forwarder

2. **WebSocket tunnel** (`src/proxy/websocket.rs`):
   - Use `hyper::upgrade::on()` to get client upgraded connection
   - Forward upgrade request to upstream
   - Use `hyper::upgrade::on()` for upstream upgraded connection
   - `tokio::io::copy_bidirectional()` between both `Upgraded` connections
   - Must call `.with_upgrades()` on the HTTP connection builder

3. **Connection builder** (`src/proxy/server.rs`):
   - Add `.with_upgrades()` to the `auto::Builder` so WebSocket upgrades work

4. **Loop detection**:
   - Add `X-Antra-Hops` header to forwarded requests
   - If hops >= 5, return `508 Loop Detected`

### Code to Reference

```
PLAN.md Phase 3              — Full spec with code patterns
docs/research/https.md       — WebSocket upgrade flow
Cargo.toml                   — Already has hyper with "full" features
```

### Key Code Pattern

```rust
// In http.rs handler, detect upgrade:
if is_websocket_upgrade(&req) {
    return websocket::handle_upgrade(req, state).await;
}

// In websocket.rs:
let client_upgrade = hyper::upgrade::on(&mut req);
let upstream_resp = client.request(upstream_req).await?;
let upstream_upgrade = hyper::upgrade::on(upstream_resp);

// Return 101 to client
let response = Response::builder()
    .status(101)
    .header("upgrade", "websocket")
    .header("connection", "upgrade")
    .body(Empty::new())?;

// Spawn tunnel
tokio::spawn(async move {
    let (client_io, upstream_io) = tokio::try_join!(client_upgrade, upstream_upgrade)?;
    let mut client = TokioIo::new(client_io);
    let mut upstream = TokioIo::new(upstream_io);
    copy_bidirectional(&mut client, &mut upstream).await
});
```

### Acceptance Criteria
- [ ] WebSocket chat app works through the proxy
- [ ] Vite HMR works (if Vite is available to test)
- [ ] `X-Antra-Hops` prevents infinite loops
- [ ] `cargo build` with zero warnings

### Exclusions (DO NOT BUILD)
- ❌ No frame-level inspection
- ❌ No WebSocket compression negotiation
- ❌ No HTTP/2 Extended CONNECT
- ❌ No message filtering or modification

---

## Phase 4 — HTTPS / TLS ✅ DONE

### Verified
- CA generation with `rcgen` (self-signed root CA)
- CA stored in `~/.config/antra/ca.pem` and `ca-key.pem` (key permissions 0o600)
- Leaf certificate generation on-demand via SNI
- In-memory cert cache with disk persistence (`~/.config/antra/certs/`)
- TLS server with `tokio-rustls` + `rustls` (ring crypto provider)
- HTTP → HTTPS redirect on port 80 (301 Moved Permanently)
- `antra run` starts HTTPS on port 443 with auto-generated certs
- `antra proxy start` starts HTTPS with cert cache
- `X-Forwarded-Proto: https` for TLS-terminated requests
- `cargo build` with zero errors and zero warnings

### Implementation Notes
- Used `rustls` with `ring` crypto provider (matches rcgen's default)
- Added `x509-parser` feature to rcgen for CA reconstruction from disk
- SNI resolver implements `ResolvesServerCert` trait
- Certs are cached in memory after first generation
- Leaf certs are stored on disk for persistence across restarts

### Exclusions (DO NOT BUILD)
- ❌ No trust store modification (Phase 6)
- ❌ No certificate renewal
- ❌ No HTTP/2 ALPN

---

## Phase 5 — Domain Resolution ✅ DONE

### Verified
- `DomainResolver` trait with `register()`, `unregister()`, `status()`
- `LocalhostResolver` — no-op for `.localhost` (browser-native per RFC 6761)
- `HostsResolver` — manages `/etc/hosts` entries for `.test` domains
- `CustomResolver` — validates and manages hosts entries for custom domains
- Atomic hosts file writes (temp + rename)
- `BEGIN/END ANTRA MANAGED HOSTS` markers for safe hosts management
- Domain validation rejects public domains (google.com, github.com, etc.)
- Domain validation rejects bare `localhost` (already resolves)
- `antra run` auto-selects resolver based on domain suffix
- Cleanup removes hosts entries on exit
- `cargo build` with zero errors and zero warnings
- 15/15 unit tests pass

### Implementation Notes
- Shared hosts file logic in `resolver/hosts.rs` (read, write, atomic rename)
- Hosts entries are `127.0.0.1 <domain>` within managed block
- Managed block is created automatically if missing
- `.localhost` is always a no-op (browsers resolve natively per RFC 6761)
- `.test` and custom domains use hosts file management
- Public domain blocklist prevents accidental hijacking

### Exclusions (DO NOT BUILD)
- ❌ No local DNS server
- ❌ No dnsmasq integration
- ❌ No mDNS/Bonjour

---

## Phase 6 — Root CA Trust ✅ DONE

### Verified
- `antra trust` installs CA into OS trust store (with user prompt)
- `antra trust --status` shows correct trust state (installed/not installed)
- `antra trust --remove` removes CA from OS trust store (with user prompt)
- `os-truststore` crate handles cross-platform trust store (macOS keychain, Linux ca-certificates, Windows certutil)
- Handles `NeedsElevation`, `InteractiveAuthRequired`, `StoreToolMissing`, `Unsupported` errors
- `antra doctor` checks actual trust status
- User prompted before any system modification
- `cargo build` with zero errors and zero warnings
- 16/16 unit tests pass

### Implementation Notes
- Used `os-truststore` crate (v0.0.2) for cross-platform trust store abstraction
- Certificate identity derived from SHA-256 of DER bytes (stable, no naming needed)
- `Cert::from_pem()` validates the cert is a CA before installation
- `is_installed()` is idempotent — safe to call multiple times
- `install()` is idempotent — already-installed certs are a no-op
- `Report` enum provides `Installed`, `AlreadyInstalled`, `InstalledNotTrusted` outcomes

### Exclusions (DO NOT BUILD)
- ❌ No Firefox NSS store modification
- ❌ No Java trust store
- ❌ No silent installation

---

## Project Structure

```
antra/
├── AGENT.md              ← YOU ARE HERE
├── PLAN.md               ← Full plan with all phases
├── Cargo.toml            ← Dependencies
├── docs/
│   ├── architecture.md   ← Module design, data types, flows
│   ├── security.md       ← Threat model, safety rules
│   ├── mvp.md            ← In/out scope, definition of done
│   └── research/
│       ├── domain-resolution.md
│       ├── https.md
│       ├── process-management.md
│       ├── portless.md
│       └── crates.md
└── src/
    ├── main.rs           ← Entry point, tracing setup
    ├── cli/              ← All subcommands (Clap)
    ├── certs/            ← CA + leaf cert generation
    ├── config/           ← antra.toml + global state
    ├── daemon/           ← Background proxy process
    ├── ipc/              ← CLI ↔ daemon communication
    ├── platform/         ← macOS/Linux/Windows abstractions
    ├── process/          ← Child process spawning + signals
    ├── proxy/            ← HTTP/HTTPS/WebSocket proxy
    ├── resolver/         ← Domain → 127.0.0.1 resolution
    ├── routing/          ← Route registry + types
    ├── trust/            ← OS trust store (install/remove/check CA)
    └── util/             ← Port allocation, terminal output
```

---

## Documentation Reference

| File | When to Read |
|------|-------------|
| `PLAN.md` | Before starting any phase — full spec + code patterns |
| `docs/architecture.md` | When implementing modules — data types, flows |
| `docs/security.md` | When touching hosts, trust store, or domains |
| `docs/mvp.md` | When unsure about scope — what's in/out |
| `docs/research/*.md` | When you need background on a specific area |

---

## After Each Phase

1. `cargo build` — zero errors, zero warnings
2. Manual test — verify the feature works end-to-end
3. Update status table in this file
4. Mark phase as ✅ DONE
5. Read the next phase's spec in PLAN.md
6. Update the "Current state" line at the top

---

## Quick Command Reference

```bash
# Build & run
cargo build
cargo run -- <args>

# Test
cargo test

# Clean build
cargo clean && cargo build

# Check without building
cargo check

# Clippy lints
cargo clippy

# Format
cargo fmt
```
