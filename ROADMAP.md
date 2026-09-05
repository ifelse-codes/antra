# Antra Roadmap — Feature Plan

> Based on portless analysis + UX test findings.
> Status: `NOW` = approved, start immediately | `NEXT` = next sprint | `LATER` = future planning

---

## Approved Now

| # | Feature | Status | Description | Effort | Notes |
|---|---------|--------|-------------|--------|-------|
| 1 | Zero-Config `antra dev` | **NOW** | Detect `package.json` / `Cargo.toml` / `go.mod` / `pyproject.toml` etc. Infer app name, infer "dev" command, auto-assign port. Run `antra dev` bare with zero flags. Language-agnostic detection, not Node-only. | Large | Must support: Node (package.json), Rust (Cargo.toml), Go (go.mod), Python (pyproject.toml/package.json), Ruby (Gemfile), Elixir (mix.exs), PHP (composer.json). Fallback to directory name. |
| 2 | PORT + HOST Env Injection | **NOW** | Always assign a free port (4000-4999). Inject `PORT`, `HOST=127.0.0.1`, `ANTRA_URL`, `ANTRA_DOMAIN` into child process. Parse command to detect framework and inject `--port` flag when framework ignores `PORT` env. | Medium | Frameworks to detect: Vite, Astro, React Router, Angular, Expo, React Native, Next.js (respects PORT), Express (respects PORT), Nuxt (respects PORT). |
| 3 | `NODE_EXTRA_CA_CERTS` Injection | **NOW** | Set `NODE_EXTRA_CA_CERTS=<ca-cert-path>` in child process env so Node.js trusts the local CA automatically. No manual config needed. | Trivial | One env var. Check `certs/store.rs` for CA path. |
| 4 | Fix Install URL | **NOW** | Website `https://antra.iifelse.com/install.sh` returns HTML. README has `install` (no `.sh`). Fix routing so the script is served correctly, or use GitHub raw URL. | Trivial | Check Cloudflare Worker routing config. |
| 5 | Fix `select_resolver` Duplication | **NOW** | `select_resolver` is copy-pasted 3x in `cli/mod.rs`, `cli/run.rs`, `cli/alias.rs` with inconsistent behavior. Consolidate into one function. | Small | DRY violation causing bugs. |

---

## Next Sprint

| # | Feature | Status | Description | Effort | Notes |
|---|---------|--------|-------------|--------|-------|
| 6 | OS Service Install | **NEXT** | `antra service install\|status\|uninstall`. Register daemon as launchd (macOS), systemd (Linux), Task Scheduler (Windows). HTTPS URLs survive reboots. | Medium | Copy portless approach. Root-owned service for port 443 binding. |
| 7 | Custom TLD + Multi-Segment | **NEXT** | `antra run --tld dev.example.com` for OAuth-friendly local URLs. Multi-segment TLDs that match production domain structure. Auto-sync `/etc/hosts` for non-`.localhost` TLDs. | Medium | Critical for OAuth redirect URIs (Google, Apple reject `.localhost`). |
| 8 | Safari DNS Fix | **NEXT** | Auto-sync `/etc/hosts` for `.localhost` subdomains (Safari doesn't resolve them). Add `antra hosts sync\|clean` commands. | Small | portless does this by default. |
| 9 | `antra prune` | **NEXT** | Kill orphaned dev servers from crashed sessions. Scan route table for dead PIDs, clean up. | Small | Simple: check PID alive, remove dead routes. |
| 10 | `--force` Route Takeover | **NEXT** | Kill existing process occupying a port and take over its route. | Small | `antra run --domain myapp.localhost --force -- pnpm dev` |
| 11 | Loop Detection | **NEXT** | Detect infinite proxy loops (frontend proxying to another antra app with wrong Host header). Return clear error with fix instructions. | Medium | portless returns `508 Loop Detected`. |

---

## Future Planning

| # | Feature | Status | Description | Effort | Notes |
|---|---------|--------|-------------|--------|-------|
| 12 | LAN Mode | **LATER** | `antra run --lan` binds to `0.0.0.0`, uses mDNS (`.local`) for device discovery. Auto-detect LAN IP, follow Wi-Fi changes. | Large | Needs `avahi-utils` on Linux, `dns-sd` on macOS. |
| 13 | Monorepo Support | **LATER** | Auto-discover workspace packages from `pnpm-workspace.yaml` / `workspaces` field. One `antra.toml` at root. Each package gets `<package>.<project>.localhost`. | Large | `apps` map in config for name overrides. |
| 14 | Git Worktree Detection | **LATER** | Auto-detect `git worktree list`, prepend branch name as subdomain (`fix-ui.myapp.localhost`). Zero config. | Medium | Nice for teams with parallel feature branches. |
| 15 | Wildcard Routing | **LATER** | `antra proxy start --wildcard` — unregistered subdomains fall back to parent route. `tenant1.myapp.localhost` → `myapp` app. | Medium | Useful for multi-tenant local dev. |
| 16 | Tailscale Integration | **LATER** | `antra run --tailscale myapp -- pnpm dev` — share on tailnet. Auto-register Tailscale HTTPS certs. | Large | Requires Tailscale CLI. |
| 17 | ngrok Integration | **LATER** | `antra run --ngrok myapp -- pnpm dev` — expose to public internet. | Large | Requires ngrok CLI + auth. |
| 18 | HTTP/2 Support | **LATER** | Enable HTTP/2 multiplexing by default. Browsers limit HTTP/1.1 to 6 connections per host — bottleneck for Vite/Nuxt unbundled dev. | Medium | rustls supports ALPN. Already have TLS, just need ALPN negotiation. |
| 19 | Custom Certs | **LATER** | `antra proxy start --cert ./cert.pem --key ./key.pem` — use your own certs (e.g., from mkcert). | Small | passthrough to rustls. |
| 20 | `--no-tls` Mode | **LATER** | `antra proxy start --no-tls` — plain HTTP on port 80. Skip CA generation entirely. | Small | Useful for CI or when HTTPS isn't needed. |
| 21 | Environment Variables | **LATER** | Full env var support: `ANTRA_PORT`, `ANTRA_HTTPS`, `ANTRA_TLD`, `ANTRA_LAN`, etc. Override defaults without flags. | Small | Config-as-code friendly. |
| 22 | Process Module Completion | **LATER** | Implement `src/process/runner.rs`, `monitor.rs`, `signals.rs` (currently stubs). Proper process lifecycle management. | Large | Currently inlined in `cli/run.rs`. |
| 23 | Streaming Response Forwarding | **LATER** | Forward upstream response body as stream instead of buffering entire body in memory. | Medium | `forward.rs` currently collects full body. Bad for large uploads/downloads. |
| 24 | WebSocket Upgrade Timeout | **LATER** | Add read timeout to WebSocket upgrade response parsing. Currently runs indefinitely. | Small | `websocket.rs` lines 112-125. |
| 25 | Continuous Port Sync | **DONE** | Monitor child process for port changes (e.g., Vite finds occupied port and reassigns). Silently update route table and domain mapping when port changes. | Medium | Watch `/proc/<pid>/fd` or use `nix::sys::ptrace` or poll network connections. Critical for frameworks that auto-reassign ports. |
| 26 | Package Script Wrapping | **DONE** | `antra add wrap-script <name>` — modify `package.json` scripts to inject antra run commands. Similar to portless's `npx portless add http://localhost:3000 myapp`. | Medium | Parse package.json, modify "dev" script, write back. Support `--force` to overwrite existing. |
| 27 | Smart Daemon Auto-Start | **DONE** | Daemon auto-starts when any `antra` command runs (not just `antra proxy start`). Check if daemon is running, start if not. | Small | Hook into CLI entry point, check daemon health, start if needed. |
| 28 | Zero-Config `antra add` | **DONE** | `antra add route --domain myapp.localhost --port 3000` — add route without running dev server. For existing servers already running. | Small | Just register route in daemon, skip process spawning. |
| 29 | Port Conflict Auto-Resolution | **DONE** | When assigned port is taken, automatically find next free port in range (4000-4999) and retry. Log the reassignment. | Small | Modify `find_free_port_in_range()` to retry on EADDRINUSE. |

---

## Cleanup (Do Alongside)

| # | Item | Status | Description |
|---|------|--------|-------------|
| C1 | Remove `Commands2` dead enum | **NOW** | `cli/mod.rs` line 103-110, never used. |
| C2 | Remove `#![allow(dead_code)]` | **NOW** | `src/main.rs` line 1, hides real warnings. |
| C3 | Fix socket permissions | **NOW** | `daemon/server.rs` line 81, change `0o666` → `0o600`. |
| C4 | Remove unused `proxy/server.rs` | **NOW** | Plain HTTP server never called in daemon path. |
| C5 | TTY check in `doctor` | **NOW** | `cli/doctor.rs` line 204, read stdin without TTY check. |
| C6 | Fix stale socket detection | **NEXT** | `ipc/client.rs` line 14, only checks file exists. Try connect to verify. |
