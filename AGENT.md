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

## Session Handoff — 2026-09-27 (afternoon)

**`v0.6.0` is published** (tag `v0.6.0`, PRs #14–#16, formula updated from the release's own sha256s, landing redeployed as a *production* deployment). ROADMAP #32 (`antra logs`) and #33 (shared upstream client) are done, along with the two resolver fixes (wrong-subcommand sudo hint, underscore domains) and the landing security headers + real 404. The working tree is clean. The v0.5.0 CA work it builds on is documented in `docs/security.md` (CA versioning and rotation).

**What changed in v0.6.0:**
- `antra logs [-f] [--lines N]`. The daemon's output used to go to `/dev/null` on the auto-start path, so "HTTPS server failed" existed only in the code that printed it; several user-test sessions had asked for this command. One log path now serves every writer — `util::logs` — including the launchd plist, which pointed at `~/.config/antra/daemon.log` while the CLI wrote `data_local_dir()/antra/daemon.log`: two different files on macOS. Log is truncated past 5 MiB rather than rotated. `doctor` tails the last errors.
- One pooled upstream client in `ProxyState` instead of one per request. `tests/upstream_pool.rs` counts TCP connections through the real TLS server: 4 sequential requests → 1 upstream connection, and the old per-request build → 4 (verified by temporarily reverting).
- A denied `/etc/hosts` write no longer suggests `sudo antra alias <domain> <port>` regardless of what you ran; the suggestion follows the domain suffix.
- Underscores are rejected in domain names. They were accepted, which is the same class of bug as the CA SAN: a `_` cannot appear in a `dNSName`, so the failure moved from a clear CLI error to a certificate no strict verifier accepts.
- Landing site: CSP, HSTS, `X-Frame-Options`, `nosniff`, `Referrer-Policy`, `Permissions-Policy`, and a real 404 (unknown paths returned the homepage with a 200).

**Verified this session:**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, full suite green in a disposable HOME (233 executions, 0 failures).
- `antra logs` against a live daemon prints its real output; the empty case explains itself and exits 0.
- The landing site was re-checked in a real browser after the CSP landed: Inter and JetBrains Mono still load, the Plausible tag is present, zero page errors, `/nonexistent` → 404.

**Do not redo blindly:**
- **`tests/e2e_next_sprint.sh` was never running.** `log_pass` used `((pass_count++))`, which exits 1 when the counter is 0, so under `set -e` the suite aborted after its first passing assertion — in every one of the four shell suites. Fixed in three of them (`e2e_portless_simple.sh` has no `set -e`). With that fixed the suite runs to completion and reports **9 passed / 46 failed on this machine**, all from one cause: the auto-started daemon cannot bind 443 (no root) or 8443/8080 (an unrelated `ssh` holds both here), so every daemon-dependent test fails. The blocker — the auto-start path having no port override (ROADMAP #21) — is **shipped** as of this change: set `ANTRA_PORT`/`ANTRA_HTTP_PORT` and the auto-started daemon inherits them. The suites still need wiring into CI on a runner where the chosen ports are free.
- Do not kill unrelated processes to make a test pass; 8443/8080 are held by someone else's `ssh` on this machine.
- If a test daemon lingers on 8443 after a run, it is an orphan from a temp HOME — check its open files (`lsof -p <pid> | grep antra`) before stopping it, so you do not kill the user's real daemon.
- The CA rules from the previous handoff still hold: existing installs rotate once and re-prompt; never add a SAN back to the CA.

**Reproduce the local gate:**
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
TEST_HOME=$(mktemp -d /tmp/antra-test.XXXXXX)
HOME="$TEST_HOME" CARGO_HOME=/Users/suman/.cargo RUSTUP_HOME=/Users/suman/.rustup cargo test -- --test-threads=4
rm -rf "$TEST_HOME"
```

**One trap this session cost time:** `wrangler pages deploy .` infers the branch from git. Deployed from a feature branch it creates a *branch* deployment and prints an alias URL — production does not move, and the site silently keeps serving the old installer. Deploy from `main`, or pass `--branch main`, and check the live domain afterwards rather than the deployment URL.

**Next actions:** the manual Safari + Firefox pass on `docs/mvp.md` (still a human step; the Safari-critical half is machine-checked in CI), and wiring the shell e2e suites into CI — now unblocked by ROADMAP #21 (env vars, shipped): run them with `ANTRA_PORT`/`ANTRA_HTTP_PORT` pointing at a free port.

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

**Current state:** Phases (0-10) are implemented and `v0.6.0` is published, including the CA rewrite that makes HTTPS work on Apple's TLS stack (v0.5.0) and, in v0.6.0, `antra logs`, the pooled upstream client, and the landing security headers. The working tree was clean, then gained ROADMAP #21 (env vars: `ANTRA_PORT`, `ANTRA_HTTP_PORT`, `ANTRA_TLD`, shipped with `cli::env_tests`). Still open: the formal browser checklist (Safari, Firefox) and wiring the shell e2e suites into CI (no longer blocked — the daemon's ports are now configurable via env).

### Landing Page

- **Location:** `landing/index.html`
- **Deployed to:** Cloudflare Pages → `https://antra.iifelse.com`
- **Project name:** `antra-landing`
- **Design language:** Mudra (dark, surgical, violet accent)
- **To update:** run `wrangler pages deploy . --project-name antra-landing` **from `landing/`** — from the repo root it would publish the whole tree, source and `target/` included
- **Last deployed:** 2026-09-27, deployment `a6908b4f` (v0.6.0 assets). Verified after the fact against the live domain: `/install.sh` returns `Content-Type: text/plain` and serves the v0.6.0 pin, the index pin example reads v0.6.0, unknown paths return 404, and the security headers are present

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
- Each phase in this file has explicit **Exclusions (DO NOT BUILD)** — follow them
- Do not add features not in the current phase

### Process
- Build compiles with zero errors and zero warnings before moving to next phase
- Test manually after each phase
- Update this file's status table when a phase completes
- Do not accumulate untested changes
- `landing/install.sh` must stay byte-identical to the root `install.sh` — it is the copy users actually download. The two drifted during the v0.5.0 release (only the version-pin comments), and nothing caught it. If you touch one, `cp install.sh landing/install.sh` and re-deploy the site with `wrangler pages deploy . --project-name antra-landing`.
- A version bump touches `Cargo.toml`, `Cargo.lock`, `README.md` (badge + status line), `install.sh` **and** `landing/install.sh`, `landing/index.html` (pin example), and `Formula/antra.rb` (needs the sha256s from the published release, so it is a separate PR *after* the release).

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
- CA generation with `rcgen` (self-signed root CA) — **no `subjectAltName`**, since v0.5.0
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
- `x509-parser` is a **direct** dependency since v0.5.0 (`certs::validate`); the rcgen feature remains for CA reconstruction from disk
- SNI resolver implements `ResolvesServerCert` trait
- Certs are cached in memory after first generation; a cached leaf inside its 45-day renewal window is dropped and re-minted
- Leaf certs are stored on disk for persistence across restarts, under a `LEAF_VERSION` marker that purges them when the format or the signing CA changes

### Exclusions (DO NOT BUILD)
- ❌ No trust store modification (Phase 6)
- ❌ No remote/ACME renewal — leafs are re-minted locally (superseded by v0.5.0, which renews inside a 45-day window)
- ❌ No HTTP/2 ALPN (superseded by v0.4.0, which negotiates H2 to the client)

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
├── README.md             ← User docs (install, commands, env vars, security)
├── ROADMAP.md            ← Feature status (done / now / next / later)
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
    ├── certs/            ← CA + leaf generation, strict validation, versioned rotation
    ├── util/             ← Daemon log (one path, writer, tail/follow), ports, output
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
tests/
    ├── cert_strict.rs       ← Strict X.509 rules (no CA SAN, validity, EKU)
    ├── e2e_securetransport.rs ← macOS: Apple's TLS stack vs a live daemon
    └── common/              ← Disposable HOME harness shared by the e2e suites
```

---

## Documentation Reference

| File | When to Read |
|------|-------------|
| `README.md` | User-facing: install, commands, env vars, security policy |
| `ROADMAP.md` | What is done / approved next / future — status column is current |
| `docs/architecture.md` | When implementing modules — data types, flows |
| `docs/security.md` | When touching hosts, trust store, or domains (includes CA versioning/rotation) |
| `docs/mvp.md` | When unsure about scope — what's in/out |
| `docs/research/*.md` | When you need background on a specific area |

---

## After Each Phase

1. `cargo build` — zero errors, zero warnings
2. Manual test — verify the feature works end-to-end
3. Update status table in this file
4. Mark phase as ✅ DONE
5. Update the "Current state" line at the top

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
