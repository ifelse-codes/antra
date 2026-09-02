<p align="center">
  <img src="https://img.shields.io/badge/antra-local%20dev%20proxy-0ea5e9?style=for-the-badge" alt="Antra">
</p>

<h1 align="center">Antra</h1>

<p align="center">
  <strong>Stable HTTPS domains for local development.</strong><br>
  No ports. No <code>/etc/hosts</code>. No certificate warnings.
</p>

<p align="center">
  <a href="https://github.com/ifelse-codes/antra/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/ifelse-codes/antra/ci.yml?branch=main&style=flat-square" alt="CI"></a>
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="MIT">
  <img src="https://img.shields.io/badge/rust-native-dea584?style=flat-square&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/version-0.1.0-0ea5e9?style=flat-square" alt="Version">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-111827?style=flat-square" alt="Platforms">
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="#cli">CLI</a> ·
  <a href="#security">Security</a> ·
  <a href="docs/architecture.md">Architecture</a>
</p>

---

```bash
antra run --domain myapp.localhost -- pnpm dev
```

```
ANTRA

✓ Domain resolved: myapp.localhost
✓ Proxy ready
✓ HTTPS ready
✓ Route registered

  → https://myapp.localhost
```

Open the URL. Your app is there. Vite HMR still works. Cookies are a secure context. Nobody typed `:5173`.

That is the whole product.

---

## The problem

Local development still looks like 2009.

| You wanted | You got |
|---|---|
| A URL | `localhost:5173` |
| HTTPS | A red lock and a "proceed anyway" click |
| Multiple services | A spreadsheet of ports |
| Cookies / Service Workers / WebCrypto | "This is not a secure context" |
| A teammate to hit the same app | "wait, which port was auth on?" |

The workaround stack is worse than the problem: edit `/etc/hosts`, run `mkcert`, write a Caddyfile, remember to trust a CA, then watch HMR break because the proxy doesn't tunnel WebSockets.

**Antra replaces that pile with one command.**

---

## Quick start

### Install (one command)

**macOS / Linux:**

```bash
curl -fsSL https://antra.iifelse.com/install | bash
```

**Homebrew:**

```bash
brew install ifelse-codes/antra/antra
```

**From source:**

```bash
git clone https://github.com/ifelse-codes/antra.git && cd antra
cargo install --path .
```

Or grab a release binary from [Releases](https://github.com/ifelse-codes/antra/releases).

### Trust the local CA (one-time, prompted automatically on first run)

```bash
antra trust
```

Antra generates a local Root CA and installs it into the **system** trust store. You will be prompted. It never installs silently.

### Run anything

```bash
antra run --domain myapp.localhost -- pnpm dev
antra run --domain api.localhost -- cargo run
antra run --domain docs.localhost -- python -m http.server
```

Visit `https://myapp.localhost`. Done.

`.localhost` is resolved by every modern browser. Antra does **not** touch `/etc/hosts` for it.

---

## Why Antra

| | What you get |
|---|---|
| **Real domains** | `https://myapp.localhost`, not `localhost:3000` |
| **HTTPS by default** | Local CA, SNI, on-demand leaf certs. No browser warning after `antra trust` |
| **Zero hosts edits for `.localhost`** | Browser-native resolution, secure context, works offline |
| **WebSocket / HMR** | Transparent `copy_bidirectional` tunnel. Vite, Next, Rails, whatever |
| **Language-agnostic** | If it binds a port, Antra can front it |
| **Daemon, not a snowflake process** | First `antra run` starts the proxy. More apps just register routes |
| **Project config** | Drop an `antra.toml`, then `antra dev` |
| **Honest security** | No telemetry. No cloud. No silent trust-store writes |

Built in Rust. TLS via `rustls`. Certs via `rcgen`. No OpenSSL. No Node runtime. No account.

---

## How it works

```
Browser
   │  https://myapp.localhost
   ▼
┌──────────────────────────────────────────┐
│  Antra daemon                            │
│                                          │
│  :80   HTTP → HTTPS redirect             │
│  :443  TLS termination (SNI → leaf cert) │
│                                          │
│  Route table                             │
│    myapp.localhost  →  127.0.0.1:5173    │
│    api.localhost    →  127.0.0.1:8080    │
└──────────────────────┬───────────────────┘
                       │  X-Forwarded-*
                       ▼
                  Your process
```

1. **CLI** spawns your command and injects `PORT`, `HOST`, `ANTRA_DOMAIN`, `ANTRA_URL`.
2. **Daemon** (auto-started) terminates TLS on `:443` and looks up the `Host` header.
3. **Certificate cache** mints a leaf cert for that SNI name, signed by the Antra CA.
4. **Proxy** forwards HTTP and tunnels WebSocket upgrades to `127.0.0.1:<port>`.
5. **Ctrl+C** kills the child process group, unregisters the route, and exits with the child's code.

Routes live in memory (`RwLock<HashMap>`). The hot path never touches disk.

Deep dive: [`docs/architecture.md`](docs/architecture.md)

---

## Domains

| Suffix | Resolution | Hosts file | Notes |
|---|---|---|---|
| `*.localhost` | Browser-native | Never | **Default. Prefer this.** Secure context. Offline. |
| `*.test` | Managed hosts block | Yes | IANA reserved. Needs `antra trust` for HTTPS. |
| `*.internal` / `*.local` | Managed hosts block | Yes | Allowed, with a warning. |
| Custom | Managed hosts block | Yes | Requires `--allow-custom-domain`. |
| Known public names (`google.com`, `github.com`, …) | — | — | **Rejected.** |

Hosts writes are atomic, scoped to a `# BEGIN ANTRA MANAGED HOSTS` block, and never clobber the rest of the file.

```bash
# Safe default
antra run --domain app.localhost -- pnpm dev

# IANA reserved, hosts-managed
antra run --domain app.test -- pnpm dev

# Explicit opt-in for everything else
antra run --domain app.internal --allow-custom-domain -- pnpm dev
```

---

## CLI

```text
antra run      Run a command behind a proxied domain
antra dev      Run from antra.toml
antra list     Active routes (domain, port, pid, uptime)
antra open     Open a domain in the default browser
antra alias    Map a domain to an already-running port
antra remove   Drop a route / alias
antra trust    Install / status / remove the local CA
antra doctor   Diagnose CA, trust, daemon, ports 80 & 443
antra proxy    start | stop | status
antra clean    Wipe Antra state (with confirmation)
```

### `antra run`

```bash
antra run --domain myapp.localhost -- pnpm dev
antra run --domain myapp.localhost --port 5173 -- pnpm dev
```

| Flag | Purpose |
|---|---|
| `--domain` | Hostname to serve (required) |
| `--port` | Upstream port. Auto-allocated if omitted |
| `--allow-custom-domain` | Permit non-`.localhost` / non-`.test` names |
| `-- <command>` | The process to spawn. Required. |

Injected environment:

```text
PORT=5173
HOST=127.0.0.1
ANTRA_DOMAIN=myapp.localhost
ANTRA_URL=https://myapp.localhost
```

Point your framework at `HOST` + `PORT` and forget the rest.

### `antra alias`

Front a process you already started:

```bash
antra alias api.localhost 8080
# → https://api.localhost
```

### `antra proxy`

```bash
antra proxy start
antra proxy start --port 443 --http-port 80 --route app.localhost:5173
antra proxy status
antra proxy stop
```

The daemon starts itself on the first `antra run`. You only need these commands when you want it explicit.

### `antra trust`

```bash
antra trust              # install CA (always prompts)
antra trust --status
antra trust --remove
```

Installing a root CA is a trust-store change. Antra treats it that way: explain, prompt, make it reversible. See [`docs/security.md`](docs/security.md).

### `antra doctor`

Checks CA presence, system trust, daemon health, route count, and whether `:80` / `:443` are bindable. Prints the fix, not a stack trace.

---

## Project config

```toml
# antra.toml
domain = "myapp.localhost"

[server]
command = "pnpm"
args = ["dev"]
port = 5173
```

```bash
antra dev
```

Precedence: **CLI flags > `antra.toml` > defaults.**

---

## What Antra is not

Antra is a local networking layer. It is not a platform.

- No cloud. No accounts. No telemetry.
- No tunnels to the public internet (use ngrok / Cloudflare Tunnel for that).
- No Docker orchestration.
- No GUI.
- No framework-specific launchers. Your command is the integration.
- No silent modification of `/etc/hosts` or the system trust store.

If a feature needs a signup, it does not belong here.

---

## Antra vs the usual suspects

| | `localhost:port` | mkcert + Caddy | ngrok | Antra |
|---|---|---|---|---|
| Stable local domain | | Manual | Random / paid | `.localhost` / `.test` |
| HTTPS without warnings | | Manual | Yes | `antra trust` once |
| Hosts file | n/a | You edit it | No | Only for `.test` / custom |
| WebSocket / HMR | Yes | Config-dependent | Yes | Transparent |
| Offline | Yes | Yes | No | Yes |
| Language-agnostic | Yes | Yes | Yes | Yes |
| Multi-app routing | DIY | Config file | Extra tunnels | `antra run` × N |
| Cloud account | No | No | Yes | No |
| One-command UX | | | Close | **Yes** |

Use ngrok when someone on another network needs your app. Use Antra when *you* need your app to feel like production on your laptop.

---

## Architecture, in one page

```
src/
├── cli/         run, dev, list, doctor, trust, proxy, alias, open, clean
├── proxy/       HTTP, HTTPS/SNI, WebSocket tunnel, X-Forwarded-*
├── certs/       Root CA, leaf certs, memory + disk cache
├── routing/     In-memory route registry
├── resolver/    .localhost (no-op) · .test / custom (hosts)
├── process/     Spawn, env inject, signal forwarding, cleanup
├── daemon/      Background proxy, idle shutdown
├── ipc/         Unix socket / Windows named pipe, versioned JSON
├── config/      antra.toml + ~/.config/antra/
└── platform/    macOS · Linux · Windows
```

| Decision | Choice | Why |
|---|---|---|
| Default TLD | `.localhost` | Browser-native, secure context, no hosts file |
| TLS | `rustls` + `tokio-rustls` | Memory-safe, no OpenSSL |
| CA | `rcgen` | Rust-native cert generation |
| WebSocket | raw bidirectional tunnel | HMR just works |
| Routes | in-memory `RwLock<HashMap>` | No disk I/O on the request path |
| IPC | Unix socket / named pipe | Platform-native, local-only |
| Trust store | `os-truststore` | Cross-platform, honest errors |

Full plan and exclusions: [`PLAN.md`](PLAN.md)

---

## Security

Antra runs as you, on your machine, and it *does* change system configuration when you ask it to. The threat model is written down, not implied.

| Risk | Guardrail |
|---|---|
| Hijacking `google.com` locally | Known public domains are rejected |
| Custom production-like names | `--allow-custom-domain` required |
| CA private key leak | `~/.config/antra/ca-key.pem` at `0600`, never logged, never sent over IPC |
| Hosts-file corruption | Writes only inside a managed block, temp-file + rename |
| Silent root-cert install | Always prompt. Always reversible via `antra trust --remove` |
| Orphan processes | Child in its own process group; SIGTERM then a grace period |
| Proxy loops | `X-Antra-Hops`, 508 after 5 |

CA install is MITRE ATT&CK T1553.004. We say that out loud so you can decide.

Read [`docs/security.md`](docs/security.md) before running `antra trust` on a shared machine.

---

## Install, in full

**One-liner (macOS / Linux):**

```bash
curl -fsSL https://antra.iifelse.com/install | bash
```

**Homebrew:**

```bash
brew install ifelse-codes/antra/antra
```

**From source:**

```bash
git clone https://github.com/ifelse-codes/antra.git && cd antra
cargo install --path .
antra --help
```

**Release binaries** (GitHub Releases, tagged `v*`)

| Target | Artifact |
|---|---|
| macOS ARM64 | `antra-aarch64-apple-darwin` |
| macOS x86_64 | `antra-x86_64-apple-darwin` |
| Linux x86_64 | `antra-x86_64-linux` |
| Linux ARM64 | `antra-aarch64-linux` |
| Windows x86_64 | `antra-x86_64-windows.exe` |

Privileged ports (`:80`, `:443`) need permission. On macOS / Linux that usually means running the daemon with enough rights to bind them, or changing the ports:

```bash
antra proxy start --port 8443 --http-port 8080
```

Then open `https://myapp.localhost:8443` if you are not on 443.

---

## Develop Antra

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check

cargo run -- --help
cargo run -- run --domain myapp.localhost -- pnpm dev
cargo run -- doctor
```

CI runs the same matrix on macOS, Ubuntu, and Windows ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)).

If you are contributing, start with [`AGENT.md`](AGENT.md) — architecture, phase history, and the rules we do not break.

---

## Status

Phases 0–10 are complete. Antra is a working local proxy: HTTP, HTTPS, WebSockets, domain resolution, CA trust, daemon/IPC, DX commands, `antra.toml`, and cross-platform builds.

This is `0.1.0`. APIs can still move. The promise will not: **one command, a real HTTPS URL, your process unchanged.**

MVP definition: [`docs/mvp.md`](docs/mvp.md)

---

## Contributing

Issues and PRs are welcome.

1. Keep it local. No cloud features.
2. Never silently mutate the trust store or `/etc/hosts`.
3. `cargo clippy -- -D warnings` and `cargo test` must pass.
4. If it is not in [`PLAN.md`](PLAN.md) or [`docs/mvp.md`](docs/mvp.md), talk first.

---

## License

MIT. Declared in [`Cargo.toml`](Cargo.toml).

---

<p align="center">
  <strong>Your app deserves a real URL, even on your laptop.</strong><br>
  <sub>If Antra deletes a line from your daily ritual, star the repo so the next person finds it.</sub>
</p>
