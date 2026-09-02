# Architecture

## Overview

Antra is a native Rust developer CLI and local networking layer that maps stable domain names to local development servers. It consists of:

1. **CLI** — Parses commands, orchestrates child processes
2. **Proxy Server** — HTTP/HTTPS reverse proxy with WebSocket support
3. **Certificate Manager** — Local CA + dynamic SNI certificate generation
4. **Route Registry** — Maps domain names to local ports
5. **Domain Resolver** — Ensures domain names resolve to 127.0.0.1
6. **Process Runner** — Spawns and monitors child processes

## Request Flow

```
Browser
   │
   │ GET https://myapp.localhost/
   │ Host: myapp.localhost
   │
   ▼
┌──────────────────────────────────────┐
│ Antra Proxy (port 443)               │
│                                      │
│ 1. TLS ClientHello (SNI: myapp.localhost)
│ 2. SNI Resolver → leaf cert          │
│ 3. TLS handshake complete            │
│ 4. HTTP GET /                        │
│ 5. Route lookup: myapp.localhost     │
│ 6. Found → 127.0.0.1:5173           │
│ 7. Forward with X-Forwarded-*        │
│                                      │
└──────────────┬───────────────────────┘
               │
               ▼
          localhost:5173
               │
               ▼
            Your App
```

## Module Structure

```
src/
├── main.rs              # Entry point, CLI dispatch
├── cli/
│   ├── mod.rs
│   ├── run.rs           # `antra run` subcommand
│   ├── list.rs          # `antra list`
│   ├── doctor.rs        # `antra doctor`
│   ├── trust.rs         # `antra trust`
│   ├── proxy.rs         # `antra proxy start|stop|status`
│   ├── clean.rs         # `antra clean`
│   └── alias.rs         # `antra alias`
├── config/
│   ├── mod.rs
│   ├── project.rs       # antra.toml parsing
│   └── global.rs        # ~/.config/antra/ state
├── daemon/
│   ├── mod.rs
│   ├── server.rs        # Main daemon loop
│   └── shutdown.rs      # Graceful shutdown
├── proxy/
│   ├── mod.rs
│   ├── http.rs          # HTTP handler + HTTPS redirect
│   ├── https.rs         # TLS termination, SNI dispatch
│   ├── websocket.rs     # WebSocket upgrade + tunnel
│   ├── forward.rs       # Reverse proxy logic
│   └── headers.rs       # X-Forwarded-* management
├── routing/
│   ├── mod.rs
│   ├── registry.rs      # In-memory route table
│   └── types.rs         # Route, Protocol, etc.
├── process/
│   ├── mod.rs
│   ├── runner.rs        # Spawn child, inject env
│   ├── monitor.rs       # Health check, port detection
│   └── signals.rs       # Signal forwarding
├── certs/
│   ├── mod.rs
│   ├── ca.rs            # Root CA generation (rcgen)
│   ├── leaf.rs          # Leaf cert generation
│   ├── cache.rs         # In-memory + disk cert cache
│   └── store.rs         # ~/.config/antra/ cert storage
├── resolver/
│   ├── mod.rs
│   ├── traits.rs        # DomainResolver trait
│   ├── localhost.rs     # .localhost (no-op)
│   ├── test.rs          # .test (hosts file)
│   └── custom.rs        # Custom domains
├── ipc/
│   ├── mod.rs
│   ├── protocol.rs      # Message types
│   ├── client.rs        # CLI → daemon
│   └── server.rs        # Daemon side
├── platform/
│   ├── mod.rs
│   ├── macos.rs
│   ├── linux.rs
│   └── windows.rs
└── util/
    ├── mod.rs
    ├── port.rs          # Port allocation
    └── output.rs        # Terminal formatting
```

## Core Data Types

```rust
// routing/types.rs

#[derive(Debug, Clone)]
pub struct Route {
    pub domain: String,
    pub host: IpAddr,
    pub port: u16,
    pub pid: Option<u32>,
    pub protocol: Protocol,
    pub created_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http,
    Https,
}

// routing/registry.rs

pub struct RouteRegistry {
    routes: RwLock<HashMap<String, Route>>,
}

// certs/cache.rs

pub struct CertCache {
    certs: RwLock<HashMap<String, CertifiedKey>>,
    path: PathBuf,  // ~/.config/antra/certs/
}

// certs/ca.rs

pub struct CaManager {
    cert: CertificateDer<'static>,
    key: SigningKeyDer<'static>,
    path: PathBuf,
}
```

## WebSocket Flow

```
Browser                    Antra                    Upstream
   │                         │                         │
   │ GET / (Upgrade: ws)     │                         │
   │────────────────────────>│                         │
   │                         │ GET / (Upgrade: ws)     │
   │                         │────────────────────────>│
   │                         │    101 Switching        │
   │                         │<────────────────────────│
   │    101 Switching        │                         │
   │<────────────────────────│                         │
   │                         │                         │
   │  ═══ copy_bidirectional ══════════════════════    │
   │                         │                         │
   │  WebSocket frames       │  WebSocket frames       │
   │────────────────────────>│────────────────────────>│
   │<────────────────────────│<────────────────────────│
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.53 | Async runtime |
| hyper | 1.11 | HTTP server/client |
| hyper-util | 0.1.20 | Server utilities |
| http-body-util | 0.1 | Body combinators |
| rustls | 0.23.43 | TLS |
| tokio-rustls | 0.26.4 | Async TLS |
| rcgen | 0.14.9 | Cert generation |
| clap | 4.6.6 | CLI parsing |
| tower | 0.5.2 | Middleware |
| tower-http | 0.7.0 | HTTP middleware |
| tracing | 0.1.44 | Structured logging |
| serde | 1.0.228 | Serialization |
| toml | 0.9.8 | TOML config |
| anyhow | 1 | Error handling |
| dirs | 5 | Config dirs |
| colored | 2 | Terminal colors |
| nix | 0.29 | Unix APIs |
