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
✅ macOS support (primary)
✅ Linux support (best effort)
✅ Colored terminal output
✅ Structured logging (tracing)
✅ antra doctor diagnostics
✅ antra list routes
✅ antra trust (CA installation)
✅ antra clean (state removal)
✅ antra proxy start|stop|status
✅ antra alias (static routes)
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
❌ Certificate renewal (dev certs are long-lived)
❌ Windows support (deferred to Phase 10)
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
- [ ] Vite HMR works (edit file → browser updates)
- [ ] WebSocket connection established (check DevTools Network tab)
- [ ] Ctrl+C terminates app and removes route
- [ ] `antra list` shows the active route
- [ ] No orphan processes after Ctrl+C
- [ ] No stale entries in `/etc/hosts`
- [ ] Same workflow works with `cargo run` and `python app.py`

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
