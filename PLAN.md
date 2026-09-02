# ANTRA — PLAN

> Single source of truth. Every decision, every phase, every exclusion.
> If it's not in this doc, we don't build it.

## Quick Reference

| Item | Value |
|------|-------|
| Product | Antra — local dev proxy CLI |
| Language | Rust 1.96 |
| License | MIT |
| MVP Target | `antra run --domain myapp.localhost -- pnpm dev` → https://myapp.localhost |
| Default Domain | `.localhost` (browser-native, no hosts modification) |

## Architecture Summary

```
BROWSER
   │ https://myapp.localhost
   ▼
┌──────────────────────────────────┐
│ ANTRA DAEMON                     │
│                                  │
│  Port 80  → HTTP→HTTPS redirect  │
│  Port 443 → TLS termination      │
│             │                    │
│             ▼                    │
│  Route Registry (in-memory)      │
│  Host: myapp.localhost           │
│       → 127.0.0.1:5173           │
│                                  │
│  Certificate Cache               │
│  SNI → signed leaf cert          │
└──────────┬───────────────────────┘
           │ TCP
           ▼
      localhost:5173
           │
           ▼
        YOUR APP
```

## Key Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Default TLD | `.localhost` | Browser-native resolution, secure context, no hosts file |
| HTTPS | First-class from Phase 4 | Required for .test, custom domains, HTTP/2 perf |
| CA generation | `rcgen` (Rust native) | No openssl dependency |
| TLS | `rustls` + `tokio-rustls` | Memory safe, fast |
| WebSocket | `copy_bidirectional` tunnel | Transparent, minimal overhead, HMR works |
| Route storage | In-memory `RwLock<HashMap>` | Fast, no disk I/O per request |
| Trust store | `os-truststore` crate | Cross-platform, honest error handling |
| IPC | Unix socket / Named pipe | Platform-native |
| Daemon | Background process, auto-start | Zero-config UX |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 0 | Project Scaffolding & Documentation | ✅ Done |
| 1 | Minimal HTTP Reverse Proxy | ✅ Done |
| 2 | CLI Process Runner | ✅ Done |
| 3 | WebSocket / HMR Support | ✅ Done |
| 4 | HTTPS / TLS | ✅ Done |
| 5 | Domain Resolution | ✅ Done |
| 6 | Root CA Trust | ✅ Done |
| 7 | Daemon + IPC | ✅ Done |
| 8 | Route Management & DX | ✅ Done |
| 9 | Configuration | ✅ Done |
| 10 | Cross-Platform Hardening | ✅ Done |
| 11 | Distribution & Developer Experience | ✅ Done |

---

## Phase 0 — Project Scaffolding & Documentation

### Goal
Set up the project, establish architecture, capture all research, create a compilable skeleton.

### Tasks
- [ ] Initialize Cargo project with binary target `antra`
- [ ] Configure Cargo.toml with all dependencies
- [ ] Write PLAN.md (this file)
- [ ] Write docs/architecture.md
- [ ] Write docs/security.md
- [ ] Write docs/mvp.md
- [ ] Write docs/research/domain-resolution.md
- [ ] Write docs/research/https.md
- [ ] Write docs/research/process-management.md
- [ ] Write docs/research/portless.md
- [ ] Write docs/research/crates.md
- [ ] Create src/ module skeleton with mod.rs stubs
- [ ] Implement CLI skeleton with Clap (all subcommands parsed, no logic)
- [ ] Verify `cargo build` compiles

### Acceptance Criteria
- [ ] `cargo build` succeeds with zero errors
- [ ] `antra --help` prints all subcommands
- [ ] `antra run --help` shows domain and command arguments
- [ ] All docs exist and are referenced from PLAN.md

### Exclusions (DO NOT BUILD)
- ❌ No actual proxy logic
- ❌ No TLS/certificate code
- ❌ No process spawning
- ❌ No hosts file modification
- ❌ No IPC
- ❌ No daemon

---

## Phase 1 — Minimal HTTP Reverse Proxy

### Goal
Antra proxies HTTP requests from a domain to a local port.

### Tasks
- [ ] Implement `RouteRegistry` with `RwLock<HashMap<String, Route>>`
- [ ] Implement `Route` and related types in `routing/types.rs`
- [ ] Implement hyper HTTP server in `proxy/http.rs`
- [ ] Implement reverse proxy forwarding in `proxy/forward.rs`
- [ ] Add `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` headers
- [ ] Implement route lookup by `Host` header
- [ ] Return 502 Bad Gateway when no route matches
- [ ] Return 503 Service Unavailable when upstream is down

### Code Patterns

```rust
// routing/types.rs
#[derive(Debug, Clone)]
pub struct Route {
    pub domain: String,
    pub host: IpAddr,
    pub port: u16,
    pub pid: Option<u32>,
    pub created_at: Instant,
}

// routing/registry.rs
pub struct RouteRegistry {
    routes: RwLock<HashMap<String, Route>>,
}

impl RouteRegistry {
    pub fn register(&self, route: Route) -> Result<()> { ... }
    pub fn unregister(&self, domain: &str) -> Result<()> { ... }
    pub fn lookup(&self, domain: &str) -> Option<Route> { ... }
    pub fn list(&self) -> Vec<Route> { ... }
}

// proxy/forward.rs
async fn forward_request(
    req: Request<Incoming>,
    route: Route,
) -> Result<Response<Full<Bytes>>> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(format!("{}:{}", route.host, route.port))
        .path_and_query(req.uri().path_and_query().unwrap().clone())
        .build()?;

    let mut upstream_req = Request::builder()
        .method(req.method())
        .uri(uri)
        .body(req.into_body())?;

    // Add forwarded headers
    upstream_req.headers_mut().insert(
        "X-Forwarded-For",
        "127.0.0.1".parse()?,
    );

    let client = Client::new();
    client.request(upstream_req).await
}
```

### Acceptance Criteria
- [ ] `antra proxy start` launches HTTP server on port 8080
- [ ] Registering a route and sending `Host: myapp.localhost` → response from upstream
- [ ] Unknown host → 502 Bad Gateway
- [ ] Upstream down → 503 Service Unavailable

### Exclusions (DO NOT BUILD)
- ❌ No HTTPS/TLS
- ❌ No WebSocket support
- ❌ No process spawning (manual upstream for now)
- ❌ No daemon mode
- ❌ No hosts file management
- ❌ No certificate generation

---

## Phase 2 — CLI Process Runner

### Goal
`antra run --domain myapp.localhost -- pnpm dev` spawns the app and registers the route.

### Tasks
- [ ] Implement `antra run` command parsing (--domain, --port, -- <command>)
- [ ] Validate domain against safe namespaces
- [ ] Allocate port (explicit --port or OS ephemeral)
- [ ] Spawn child process with injected env vars (PORT, HOST, ANTRA_DOMAIN, ANTRA_URL)
- [ ] Register route in registry
- [ ] Monitor child process (wait for exit)
- [ ] Forward SIGINT/SIGTERM to child
- [ ] Remove route on child exit
- [ ] Exit with child's exit code

### Code Patterns

```rust
// cli/run.rs
pub async fn run(args: RunArgs) -> Result<()> {
    let port = args.port.unwrap_or(find_free_port()?);
    let route = Route {
        domain: args.domain.clone(),
        host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        pid: None,
        created_at: Instant::now(),
    };

    registry.register(route)?;

    let child = Command::new(&args.command)
        .args(&args.args)
        .env("PORT", port.to_string())
        .env("HOST", "127.0.0.1")
        .env("ANTRA_DOMAIN", &args.domain)
        .env("ANTRA_URL", format!("https://{}", args.domain))
        .spawn()?;

    // Wait for child, forward signals, cleanup on exit
    match child.wait_with_output().await {
        Ok(status) => {
            registry.unregister(&args.domain)?;
            std::process::exit(status.code().unwrap_or(1));
        }
        Err(e) => {
            registry.unregister(&args.domain)?;
            Err(e.into())
        }
    }
}
```

### Acceptance Criteria
- [ ] `antra run --domain myapp.localhost -- python -m http.server 5173` works
- [ ] `http://localhost:8080` with `Host: myapp.localhost` → proxied response
- [ ] Ctrl+C terminates child process
- [ ] Route removed after child exits
- [ ] No orphan processes

### Exclusions (DO NOT BUILD)
- ❌ No HTTPS
- ❌ No WebSocket
- ❌ No auto port detection from stdout
- ❌ No framework-specific injection

---

## Phase 3 — WebSocket / HMR Support

### Goal
WebSocket connections (including Vite HMR) tunnel through the proxy.

### Tasks
- [ ] Detect `Upgrade: websocket` in incoming requests
- [ ] Use `hyper::upgrade::on()` for client upgrade
- [ ] Forward upgrade request to upstream
- [ ] Use `hyper::upgrade::on()` for upstream upgrade
- [ ] `tokio::io::copy_bidirectional()` between both connections
- [ ] Enable `.with_upgrades()` on HTTP connection builder
- [ ] Add loop detection header `X-Antra-Hops`

### Code Patterns

```rust
// proxy/websocket.rs
pub async fn handle_upgrade(
    req: Request<Incoming>,
    route: Route,
) -> Result<Response<Empty<Bytes>>> {
    // 1. Capture client upgrade before returning response
    let client_upgrade = hyper::upgrade::on(&mut req);

    // 2. Build upstream upgrade request
    let upstream_req = build_ws_upgrade_request(&req, &route)?;

    // 3. Send to upstream
    let upstream_resp = client.request(upstream_req).await?;
    assert_eq!(upstream_resp.status(), StatusCode::SWITCHING_PROTOCOLS);

    // 4. Capture upstream upgrade
    let upstream_upgrade = hyper::upgrade::on(upstream_resp);

    // 5. Return 101 to client
    let response = Response::builder()
        .status(101)
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .body(Empty::new())?;

    // 6. Spawn tunnel
    tokio::spawn(async move {
        let (client_io, upstream_io) =
            tokio::try_join!(client_upgrade, upstream_upgrade)?;
        let mut client = TokioIo::new(client_io);
        let mut upstream = TokioIo::new(upstream_io);
        copy_bidirectional(&mut client, &mut upstream).await
    });

    Ok(response)
}
```

### Acceptance Criteria
- [ ] Vite HMR works through the proxy (edit file → browser updates)
- [ ] WebSocket chat app works through the proxy
- [ ] No data loss or framing issues
- [ ] `X-Antra-Hops` prevents infinite loops

### Exclusions (DO NOT BUILD)
- ❌ No frame-level inspection
- ❌ No WebSocket compression negotiation
- ❌ No HTTP/2 Extended CONNECT

---

## Phase 4 — HTTPS / TLS

### Goal
`https://myapp.localhost` works with no browser warnings.

### Tasks
- [ ] Generate Root CA with `rcgen`
- [ ] Store CA in `~/.config/antra/`
- [ ] Implement SNI resolver (in-memory cert cache)
- [ ] Generate leaf certificates on-demand
- [ ] TLS server with `tokio-rustls`
- [ ] HTTP → HTTPS redirect on port 80
- [ ] Certificate persistence (disk cache)

### Code Patterns

```rust
// certs/ca.rs
pub fn generate_ca() -> Result<(CertificateDer<'static>, SigningKeyDer<'static>)> {
    let mut params = CertificateParams::new(vec!["Antra Local CA".to_string()])?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let key = KeyPair::generate()?;
    let cert = params.self_signed(&key)?;
    Ok((cert, key.into()))
}

// certs/leaf.rs
pub fn generate_leaf_cert(
    hostname: &str,
    ca_key: &SigningKey,
    ca_cert: &CertificateRef,
) -> Result<(CertificateDer<'static>, SigningKeyDer<'static>)> {
    let params = CertificateParams::new(vec![hostname.to_string()])?;
    let key = KeyPair::generate()?;
    let cert = params.signed_by(&key, ca_key, ca_cert)?;
    Ok((cert, key.into()))
}

// certs/cache.rs
pub struct CertCache {
    certs: RwLock<HashMap<String, CertifiedKey>>,
    path: PathBuf,
}

impl ResolvesServerCert for CertCache {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = hello.server_name()?.to_str().to_string();
        // Check memory cache → disk cache → generate
    }
}
```

### Acceptance Criteria
- [ ] `https://myapp.localhost` loads in Chrome with no cert warning
- [ ] `https://myapp.localhost` loads in Firefox with no cert warning
- [ ] Certificate chain is valid (CA → leaf)
- [ ] New domains get certs automatically via SNI

### Exclusions (DO NOT BUILD)
- ❌ No trust store modification (Phase 6)
- ❌ No certificate renewal
- ❌ No HTTP/2 ALPN (future)

---

## Phase 5 — Domain Resolution

### Goal
Domain names resolve to 127.0.0.1 without manual hosts file editing.

### Tasks
- [ ] Implement `DomainResolver` trait
- [ ] `.localhost` strategy: no-op (browser-native)
- [ ] `.test` strategy: managed hosts file block
- [ ] Custom domain strategy: managed hosts file block
- [ ] Atomic hosts file write (temp + rename)
- [ ] Platform-specific hosts path detection
- [ ] `BEGIN/END ANTRA MANAGED HOSTS` markers
- [ ] Domain validation (reject dangerous overrides)

### Code Patterns

```rust
// resolver/traits.rs
pub trait DomainResolver: Send + Sync {
    fn register(&self, domain: &str) -> Result<()>;
    fn unregister(&self, domain: &str) -> Result<()>;
    fn status(&self, domain: &str) -> Result<ResolutionStatus>;
}

// resolver/localhost.rs
pub struct LocalhostResolver;

impl DomainResolver for LocalhostResolver {
    fn register(&self, _domain: &str) -> Result<()> {
        // No-op: browsers resolve *.localhost natively
        Ok(())
    }
    fn unregister(&self, _domain: &str) -> Result<()> { Ok(()) }
    fn status(&self, _domain: &str) -> Result<ResolutionStatus> {
        Ok(ResolutionStatus::Active)
    }
}

// resolver/test.rs
pub struct HostsResolver {
    hosts_path: PathBuf,
}

impl DomainResolver for HostsResolver {
    fn register(&self, domain: &str) -> Result<()> {
        // Add to managed block in /etc/hosts
    }
    fn unregister(&self, domain: &str) -> Result<()> {
        // Remove from managed block
    }
    fn status(&self, domain: &str) -> Result<ResolutionStatus> {
        // Check if entry exists in managed block
    }
}
```

### Acceptance Criteria
- [ ] `myapp.localhost` works in Chrome/Firefox without hosts modification
- [ ] `myapp.test` works after `antra trust` (hosts management)
- [ ] Custom domain requires explicit flag
- [ ] Known public domains are rejected

### Exclusions (DO NOT BUILD)
- ❌ No local DNS server
- ❌ No dnsmasq integration
- ❌ No mDNS/Bonjour

---

## Phase 6 — Root CA Trust

### Goal
`antra trust` installs the CA into the system trust store.

### Tasks
- [ ] Implement cross-platform trust store via `os-truststore`
- [ ] Prompt user before modifying trust store
- [ ] `antra trust` installs CA
- [ ] `antra trust --status` shows current state
- [ ] `antra trust --remove` removes CA
- [ ] Handle platform-specific errors (macOS GUI auth, Linux sudo)

### Acceptance Criteria
- [ ] After `antra trust`, `https://myapp.test` has no cert warning
- [ ] `antra trust --status` correctly reports trust state
- [ ] User is prompted before any system modification

### Exclusions (DO NOT BUILD)
- ❌ No Firefox NSS store modification
- ❌ No Java trust store
- ❌ No silent installation

---

## Phase 7 — Daemon + IPC

### Goal
Background proxy daemon, CLI communicates via IPC.

### Tasks
- [x] Implement daemon process (fork + daemonize)
- [x] Unix domain socket IPC (macOS/Linux)
- [x] Named pipe IPC (Windows)
- [x] JSON message protocol with versioning
- [x] Auto-start daemon when first app registers
- [x] Idle shutdown (configurable timeout)
- [x] `antra proxy start|stop|status` commands

### Code Patterns

```rust
// ipc/protocol.rs
#[derive(Serialize, Deserialize)]
pub struct IpcMessage {
    pub version: u32,
    pub command: IpcCommand,
}

#[derive(Serialize, Deserialize)]
pub enum IpcCommand {
    RegisterRoute { domain: String, port: u16 },
    UnregisterRoute { domain: String },
    ListRoutes,
    Ping,
    Shutdown,
}
```

### Acceptance Criteria
- [x] Daemon starts automatically on first `antra run`
- [x] CLI sends commands via IPC, daemon responds
- [x] `antra proxy status` shows running daemon info
- [x] Daemon shuts down after idle timeout

### Exclusions (DO NOT BUILD)
- ❌ No TCP IPC (Unix socket only)
- ❌ No encrypted IPC
- ❌ No multi-user support

---

## Phase 8 — Route Management & DX

### Goal
Developer-facing commands for visibility and control.

### Tasks
- [x] `antra list` — show active routes (domain, target, PID)
- [x] `antra open <domain>` — open in default browser
- [x] `antra doctor` — comprehensive diagnostics
- [x] `antra clean` — remove all Antra state
- [x] `antra alias <domain> <port>` — static routes
- [x] Improved terminal output with colored status

### Acceptance Criteria
- [x] `antra list` shows all active routes in a table
- [x] `antra doctor` identifies common issues with actionable fixes
- [x] `antra clean` removes all state with confirmation prompt

### Exclusions (DO NOT BUILD)
- ❌ No log viewer
- ❌ No web dashboard
- ❌ No export/import

---

## Phase 9 — Configuration

### Goal
Project-level config via `antra.toml`.

### Tasks
- [x] Parse `antra.toml` from project root
- [x] `antra dev` reads config and runs
- [x] Config precedence: CLI > project config > defaults
- [x] Config validation and error messages

### Code Patterns

```toml
# antra.toml
domain = "myapp.localhost"

[server]
command = "pnpm"
args = ["dev"]
port = 5173
```

### Acceptance Criteria
- [x] `antra dev` works without any CLI arguments when `antra.toml` exists
- [x] CLI flags override config file values
- [x] Missing config fields produce helpful errors

### Exclusions (DO NOT BUILD)
- ❌ No YAML/JSON config
- ❌ No config inheritance
- ❌ No config generation

---

## Phase 10 — Cross-Platform Hardening

### Goal
Test and fix platform-specific behavior on macOS, Linux, Windows.

### Tasks
- [x] Test full workflow on macOS (ARM64)
- [x] Test full workflow on Linux (Ubuntu, Fedora)
- [x] Test full workflow on Windows
- [x] Fix platform-specific issues
- [x] CI/CD with cross-compilation
- [x] Release binaries for all targets

### Acceptance Criteria
- [x] All MVP tests pass on macOS, Linux, Windows
- [x] Release binaries available for macOS ARM64/x86_64, Linux x86_64/ARM64, Windows x86_64

### Exclusions (DO NOT BUILD)
- ❌ No FreeBSD
- ❌ No Docker
- ❌ No CI-specific features

---

## Phase 11 — Distribution & Developer Experience

### Goal
One-command install. Zero-setup UX. A new user clones and runs in under 30 seconds.

### Tasks
- [x] Install script (`install.sh`) — auto-detect OS/arch, download binary, verify checksum, ask trust
- [x] Homebrew formula (`Formula/antra.rb`) — `brew install ifelse-codes/antra/antra`
- [x] Auto-trust prompt on first `antra run` — asks y/n, runs `antra trust` if yes, warns if no
- [x] `--no-trust-prompt` flag for `run` and `dev` — skip auto-trust in CI/scripts
- [x] `antra doctor` auto-fix — detects issues, offers to fix them automatically
- [x] Release workflow checksums — SHA256 for all binaries, attached to GitHub releases
- [x] Global config (`~/.config/antra/config.toml`) — tracks trust prompt state
- [x] README updated with one-liner install options

### Install Flow

```
curl -fsSL https://antra.iifelse.com/install | bash
```

1. Detect OS (macOS/Linux/Windows) and arch (arm64/x86_64)
2. Fetch latest version from GitHub releases
3. Download correct binary + SHA256 checksum
4. Verify checksum
5. Install to /usr/local/bin or ~/.local/bin
6. Prompt: "Install CA into system trust store? [y/N]"
7. If yes: run `antra trust` (prompts for sudo)
8. If no: warn + show `antra trust` command for later

### Homebrew Flow

```bash
brew install ifelse-codes/antra/antra
```

Formula downloads platform-specific binary from GitHub releases.
Caveats printed post-install show `antra trust` command.

### Auto-Trust Flow

On first `antra run` (or `antra dev`):
1. Check if CA is trusted → skip if already trusted
2. Check if prompt was already shown → skip if so (stored in `~/.config/antra/config.toml`)
3. Show prompt: explain what CA does, ask y/n
4. If y: run `antra trust` (handles sudo, prompts, errors)
5. If n: warn about cert warnings, show recovery commands
6. Mark prompt as shown (won't ask again)

### Doctor Auto-Fix Flow

`antra doctor` now:
1. Detects issues (CA missing, trust missing, daemon down, ports in use)
2. Shows each issue with a fix command
3. Asks "Auto-fix all issues? [y/N]"
4. If y: executes fix commands, reports success/failure
5. If n: shows manual commands

### Exclusions (DO NOT BUILD)
- ❌ No auto-update mechanism
- ❌ No telemetry
- ❌ No cloud account required
- ❌ No GUI installer
