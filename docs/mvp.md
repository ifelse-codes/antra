# MVP Definition

## In Scope

```
✅ Rust CLI (Clap derive)
✅ Arbitrary child commands (language-agnostic)
✅ Domain → local port routing
✅ HTTP reverse proxy
✅ HTTPS reverse proxy
✅ Local CA (rcgen, Rust-native)
✅ Safe certificate generation (rustls)
✅ WebSocket tunneling (copy_bidirectional)
✅ HMR support (via transparent WebSocket)
✅ In-memory route registry (RwLock<HashMap>)
✅ Process lifecycle (spawn, monitor, cleanup)
✅ macOS support
✅ Linux support
✅ Windows support (Phase 10)
✅ Colored terminal output
✅ Structured logging (tracing)
✅ antra doctor diagnostics
✅ antra logs (daemon log, -f to follow)
✅ antra list routes
✅ antra trust (CA installation)
✅ antra clean (state removal)
✅ antra proxy start|stop|status
✅ antra alias (static routes)
✅ antra add (route to running server)
✅ antra prune (orphan cleanup)
✅ antra hosts (Safari /etc/hosts management)
✅ antra service (OS service management)
✅ antra dev (zero-config auto-detect: Node, Rust, Go, Python, Ruby, Elixir, PHP)
✅ antra.toml project config
✅ Cross-platform platform abstraction
```

## Out of Scope (MVP)

```
❌ Browser extension
❌ Native messaging
❌ GUI / TUI
❌ Cloud service
❌ External DNS provider
❌ Account system
❌ Telemetry
❌ Framework-specific launchers
❌ Docker integration
❌ LAN mode / mDNS
❌ HTTP/2 to upstream (only to client)
❌ Remote certificate renewal (leafs are re-minted locally inside a 45-day window; nothing phones home)
❌ FreeBSD support
❌ Custom port ranges
❌ Config inheritance
❌ YAML/JSON config
```

## Definition of Done

The MVP is complete when this works reliably:

```bash
antra run --domain myapp.localhost -- pnpm dev
```

Terminal output:
```
ANTRA

✓ Proxy ready (port 443)
✓ HTTPS ready
✓ Route registered

  https://myapp.localhost
  → 127.0.0.1:5173
```

### Verification Checklist

- [ ] `https://myapp.localhost` loads in Chrome with no cert warning
- [ ] `https://myapp.localhost` loads in Firefox with no cert warning
- [ ] `https://myapp.localhost` loads in Safari with no cert warning (run `antra hosts sync` first — Safari does not resolve `*.localhost`). `antra doctor` must first report "CA passes strict X.509 validation"
- [ ] Vite HMR works (edit file → browser updates)
- [ ] WebSocket connection established (check DevTools Network tab)
- [ ] Ctrl+C terminates app and removes route
- [ ] `antra list` shows the active route
- [ ] No orphan processes after Ctrl+C
- [ ] No stale entries in `/etc/hosts`
- [ ] Same workflow works with `cargo run` and `python app.py`

**All boxes are unchecked on purpose.** This checklist is a human DoD: each row needs a real browser or a real session, which no automated gate can stand in for. What *is* machine-checked today, and by what:

| Claim | Gate |
|---|---|
| The chain is valid under Apple's TLS stack — the one that rejected the pre-0.5 root | `tests/e2e_securetransport.rs`: `/usr/bin/curl --cacert` through a live `antra proxy start` + `antra alias`, macOS CI |
| The minted CA and leafs satisfy strict X.509 rules (no CA SAN, valid `dNSName`, `serverAuth` EKU, validity window) | `tests/cert_strict.rs` |
| Upgrades migrate: old CA rotated, leafs purged, re-trust prompted, superseded root removed byte-exactly | `tests/cert_store.rs` + the manual walkthrough in `fix-plan-2026-09-26-ca-trust.md` §9 |

Still to be ticked by hand: Chrome, Firefox and Safari in a real browser (Safari needs `antra hosts sync` first), a real Vite HMR session, and the Ctrl+C cleanup rows.

### Terminal UX

```
$ antra run --domain myapp.localhost -- pnpm dev

ANTRA

✓ Proxy ready (port 443)
✓ HTTPS ready
✓ Route registered

  https://myapp.localhost
  → 127.0.0.1:5173

  vite v5.4.0 dev server ready for you to use...

  ➜  Local:   http://localhost:5173/
  ➜  Network: use --host to expose
```
