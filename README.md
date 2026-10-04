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
  <img src="https://img.shields.io/badge/version-0.6.6-0ea5e9?style=flat-square" alt="Version">
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
curl -fsSL https://antra.iifelse.com/install.sh | bash
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

### Port 443 (one-time question on first run)

A URL with no port number needs port 443, and on macOS and Linux only an
admin can open it. The first time Antra starts its proxy it asks:

```text
  Use port 443? [Y/n]
```

Say yes and `sudo` asks for your password once. The proxy opens ports 443 and
80, **then drops admin rights** and runs as you; it stays up until you restart
or run `antra proxy stop`, so you are not asked again. Say no and you get
`https://myapp.localhost:8443` instead. Antra remembers either answer.

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
| Known public names (`google.com`, `github.com`, …) | Managed hosts block | Yes | Requires `--allow-custom-domain` and prints a warning. Approval is a warning boundary, not an allowlist — see [`docs/security.md`](docs/security.md). |

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
antra dev      Run from antra.toml (or auto-detect project)
antra add      Add a route to an already-running server
antra list     Active routes (domain, port, pid, uptime)
antra open     Open a domain in the default browser
antra alias    Map a domain to an already-running port
antra remove   Drop a route / alias
antra prune    Remove routes whose process has exited
antra hosts    Manage /etc/hosts entries for Safari compatibility
antra service  Manage Antra as a system service
antra trust    Install / status / remove the local CA
antra doctor   Diagnose CA, trust, daemon, ports 80 & 443
antra logs     Read what the daemon printed (add -f to follow)
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
| `--allow-custom-domain` | Approve a custom/public-looking domain before registration |
| `--yes` | Skip the first-run CA question and explicitly install trust |
| `--no-trust-prompt` | Skip CA setup for this invocation |
| `-- <command>` | The process to spawn. Required. |

Injected environment:

```text
PORT=5173
HOST=127.0.0.1
ANTRA_DOMAIN=myapp.localhost
ANTRA_URL=https://myapp.localhost
```

Point your framework at `HOST` + `PORT` and forget the rest.

A server that ignores `PORT` — a hardcoded `listen(3000)` — still works: when
Antra had to pick the port itself and nothing answers there, it finds the
port your server really listens on (macOS and Linux), moves the route and
prints the `--port` to use next time. If the process behind a route dies
without cleaning up (`kill -9`, a closed terminal), the daemon drops its
route within seconds.

`antra run` is foreground: Ctrl-C stops the child and removes the route.
For a background server you already started, use `antra alias` instead
(no `--detach` mode — `run` never detaches).

### `antra alias`

Front a process you already started:

```bash
antra alias api.localhost 8080
# → https://api.localhost
```

Custom or public-looking names require `--allow-custom-domain`; `.localhost`, `.test`, `.local`, and `.internal` remain automatic development namespaces.

### `antra proxy`

```bash
antra proxy start
antra proxy start --port 443 --http-port 80 --route app.localhost:5173
antra proxy status
antra proxy stop
```

The daemon starts itself on the first `antra run`. You only need these commands when you want it explicit.

### `antra service`

```bash
antra service install    # Windows: sc.exe AntraDaemon (manual start, needs admin)
antra service status
antra service uninstall
```

macOS: install writes a LaunchAgent to `~/Library/LaunchAgents/com.antra.proxy.plist` that starts at login and restarts the daemon if it exits; its output goes to `antra logs`. If a daemon is already running, install says so and prints the one-line hand-over instead of starting a second one.

Linux: install writes a systemd user unit to `~/.config/systemd/user/antra-proxy.service`, enables it and starts it; its output goes to `antra logs`. It needs a systemd user session — over SSH without lingering, in WSL or in a container there may not be one, and install says so. The unit runs as you, and most systems do not let a normal user bind 443, so expect the daemon's usual fallback to 8443. `ANTRA_PORT` / `ANTRA_HTTP_PORT` set when you run install are written into the unit.

Windows limitation: the service runs as SYSTEM, so it uses the SYSTEM profile's CA and aliases — not yours. Trust and aliases you created as yourself won't apply to it (expect TLS warnings). For single-user dev, prefer `antra proxy start`.

### `antra trust`

```bash
antra trust              # install CA (always prompts)
antra trust --status
antra trust --remove
```

Installing a root CA is a trust-store change. Antra explains the change, asks `[Y/n]` on first run (Enter accepts), and keeps it reversible through `antra trust --remove` or `antra clean`. See [`docs/security.md`](docs/security.md).

### `antra doctor`

Checks CA presence and strict X.509 validity, system trust, daemon health and whether its CA is current, route count, whether `:80` / `:443` are bindable, and the last few errors the daemon logged. Prints the fix, not a stack trace.

### `antra logs`

```bash
antra logs              # last 50 lines of the daemon log
antra logs --lines 200  # more history
antra logs -f           # follow, like tail -f
```

The daemon logs everything it does here, including what used to vanish: a
failed HTTPS bind, a TLS handshake error, a route it could not resolve. Both
`antra proxy start` and the daemon Antra starts for you write to this one
file, and `antra doctor` shows the last few error lines when it finds any.

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

| | `localhost:port` | mkcert + Caddy | ngrok | portless | Antra |
|---|---|---|---|---|---|
| Stable local HTTPS URL | — | Manual | ✗ (public) | ✓ | ✓ |
| No browser warning | — | Manual | ✓ | ✓ | ✓ (Linux: one `certutil` line) |
| Works offline | ✓ | ✓ | ✗ | ✓ | ✓ |
| Traffic stays on your machine | ✓ | ✓ | ✗ | ✓ | ✓ |
| No cloud account | ✓ | ✓ | ✗ | ✓ | ✓ |
| Any language / runtime | ✓ | ✓ | ✓ | ✗ (Node 24+) | ✓ |
| Runtime to install | — | Caddy | ngrok | Node 24+ | none |
| HMR / WebSocket | ✓ | Config-dependent | ✓ | ✓ | ✓ |
| Multi-app routing | DIY | Config file | Extra tunnels | ✓ | `antra run` × N |
| Monorepo / LAN / phone | DIY | Config file | ✓ | ✓ | Roadmap |
| HTTP/2 | — | ✓ | ✓ | ✓ | ✗ |
| Asks before changing your machine | n/a | n/a | n/a | ✗ | ✓ |

Use ngrok (or a Cloudflare Tunnel) when someone on another network needs your
app. Use Antra when *you* need your app to feel like production on your laptop —
one native binary, any language, and it asks before it changes anything.

---

## Architecture, in one page

```
src/
├── cli/         run, dev, add, list, doctor, trust, proxy, alias, open, remove, prune, hosts, service, clean
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

Full plan and exclusions: [`docs/mvp.md`](docs/mvp.md)

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
| A root process parsing TLS | Under `sudo` the proxy opens 443/80 as root, then runs as your user; no long-lived root process |
| Orphan processes | Child in its own process group; SIGTERM then a grace period |
| Proxy loops | `X-Antra-Hops`, 508 after 5 |

CA install is MITRE ATT&CK T1553.004. We say that out loud so you can decide.

Read [`docs/security.md`](docs/security.md) before running `antra trust` on a shared machine.

---

## Install, in full

**One-liner (macOS / Linux):**

```bash
curl -fsSL https://antra.iifelse.com/install.sh | bash
```

**Homebrew:**

```bash
brew install ifelse-codes/antra/antra
```

**From source (requires Rust ≥ 1.85 — `serde_spanned` needs `edition2024` support):**

```bash
rustup update
rustc --version  # confirm ≥ 1.85
git clone https://github.com/ifelse-codes/antra.git && cd antra
cargo install --path .
antra --help
```

On Windows, `npm` is resolved via `PATH` plus `C:\Program Files\nodejs\`,
`%APPDATA%\npm\`, and `%LOCALAPPDATA%\Programs\nodejs\` (with `.cmd` /
`.exe` / `.bat` extension probes), so `antra run --domain myapp.localhost --
npm run dev -- --port 3001` works without a full path.

**Release binaries** (GitHub Releases, tagged `v*`)

| Target | Artifact |
|---|---|
| macOS ARM64 | `antra-aarch64-apple-darwin` |
| macOS x86_64 | `antra-x86_64-apple-darwin` |
| Linux x86_64 | `antra-x86_64-linux` |
| Linux ARM64 | `antra-aarch64-linux` |
| Windows x86_64 | `antra-x86_64-windows.exe` |

Privileged ports (`:80`, `:443`) need admin rights on macOS / Linux. The
first `antra run` offers to start the proxy with `sudo` (see
[Port 443](#port-443-one-time-question-on-first-run)); by hand it is:

```bash
antra proxy stop && sudo antra proxy start
```

Under `sudo` the proxy opens the ports, then runs as you, with your own CA,
socket and log. Without it, Antra falls back to `:8443`, or pick ports:

```bash
antra proxy start --port 8443 --http-port 8080
```

Then open `https://myapp.localhost:8443` if you are not on 443.

### Configuration via environment variables

Ports and the TLD can be set from the environment instead of flags — useful
when the daemon is started implicitly (so it inherits them) or for
config-as-code setups. An explicit flag always wins over the environment
variable.

| Variable | Default | What it sets |
|---|---|---|
| `ANTRA_PORT` | `443` | Daemon HTTPS port (`antra proxy start --port`) |
| `ANTRA_HTTP_PORT` | `80` | Daemon HTTP→HTTPS redirect port (`--http-port`) |
| `ANTRA_TLD` | — | Custom TLD for `antra run` (`--tld`) |

For example, to move the whole daemon to unprivileged ports without touching
every command:

```bash
export ANTRA_PORT=8443
export ANTRA_HTTP_PORT=8080
antra proxy start        # daemon binds :8443 / :8080
antra alias myapp.localhost 5173   # inherits the daemon on those ports
```

`ANTRA_PORT` intentionally only configures the **daemon**; the backend app
port is still auto-detected or passed with `antra run --port`.

---

## Security

### Reporting a Vulnerability

Antra takes security seriously. If you believe you have found a security
vulnerability in Antra, please **do not** file a public GitHub issue.

Send a private report instead, so the issue can be fixed before it is
disclosed:

- **GitHub:** use the [Security advisory](https://github.com/ifelse-codes/antra/security/advisories/new)
  form to create a private vulnerability report (see the maintainer's GitHub
  profile for `ifelse-codes` for a direct contact email).

We aim to acknowledge reports within **3 business days** and to provide an
initial assessment within **7 business days**. You will be kept informed of
the status of your report as it is triaged and fixed.

### Security model

Antra is a local development tool. Its two trust-sensitive surfaces are:

1. **Local Certificate Authority (CA)** — Antra generates a CA on your machine
   and, with your consent, installs it into the system trust store. Anyone
   with access to the CA private key can issue certificates that your machine
   trusts. The key is stored locally and should never leave your device.
2. **Hosts file management** — Antra writes managed entries to your OS hosts
   file, scoped to a `# BEGIN ANTRA MANAGED HOSTS` block. These changes are
   reversible via `antra clean` / `antra trust --remove`.

Neither surface involves a network service controlled by the project. Antra
has no telemetry, no accounts, and no cloud dependency. The detailed threat
model and safeguards live in [`docs/security.md`](docs/security.md).

### Scope

The following are **out of scope** and are not eligible for disclosure under
this policy:

- Theft or loss of the local CA private key due to compromise of the host
  machine itself (inherent to any local trust store).
- Social engineering of the user.
- Vulnerabilities in third-party dependencies already fixed upstream; please
  report those to the upstream project.

### Coordinating public disclosure

We appreciate coordinated disclosure. We will work with you to agree on a
timeline before public release once a fix is available. We will credit
researchers in the release notes when a confirmed vulnerability is reported
responsibly.

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

There are also four shell end-to-end suites under `tests/e2e_*.sh` that drive
the real binary. Two of them run in CI on macOS and Ubuntu; all four run
locally and are green:

```bash
# Short HOME and free ports. The daemon socket must fit the 104-byte
# sun_path limit, so a deep temp directory makes the daemon unstartable.
export HOME=/tmp/antra-shell-e2e ANTRA_PORT=18443 ANTRA_HTTP_PORT=18080
mkdir -p "$HOME"
for f in tests/e2e_*.sh; do bash "$f"; done
```

They need a built binary (`cargo build` first). Each exits non-zero only on a
real failure — a test whose toolchain is missing reports SKIP and is counted
in the summary, so `yarn`, `bun`, `python`, `mix` and `php` are optional and
installing them converts skips into passes. Give each suite its own `HOME` if
you are running more than one at a time. Results land in
`/tmp/antra-*-test-results.txt`.

If you are contributing, start with [`AGENT.md`](AGENT.md) — architecture, phase history, and the rules we do not break.

---

## Status

Phases 0–10 are complete. Antra is a working local proxy: HTTP, HTTPS, WebSockets, domain resolution, CA trust, daemon/IPC, DX commands, `antra.toml`, and cross-platform builds.

This is `0.6.6`. APIs can still move. The promise will not: **one command, a real HTTPS URL, your process unchanged.**

MVP definition: [`docs/mvp.md`](docs/mvp.md)

---

## Contributing

Issues and PRs are welcome.

1. Keep it local. No cloud features.
2. Never silently mutate the trust store or `/etc/hosts`.
3. `cargo clippy -- -D warnings` and `cargo test` must pass.
4. If it is not in [`docs/mvp.md`](docs/mvp.md) or [`ROADMAP.md`](ROADMAP.md), talk first.

---

## License

MIT. Declared in [`Cargo.toml`](Cargo.toml).

---

<p align="center">
  <strong>Your app deserves a real URL, even on your laptop.</strong><br>
  <sub>If Antra deletes a line from your daily ritual, star the repo so the next person finds it.</sub>
</p>
