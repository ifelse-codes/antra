# Security

## Threat Model

Antra runs locally and modifies system configuration (hosts file, trust store). The threat model focuses on:

1. **Accidental domain hijacking** — user routes a production domain locally
2. **CA key compromise** — leaked private key enables MITM attacks
3. **Malicious child process** — spawned app manipulates Antra state
4. **DNS rebinding** — external site accesses local services through Antra

## Safety Rules

### Domain Safety

| Domain Type | Action |
|-------------|--------|
| `*.localhost`, `localhost` | ✅ Always safe — browsers resolve natively |
| `*.test` | ✅ Always safe — IANA reserved |
| `*.internal`, `*.local` | ⚠️ Warn but allow |
| Public or other custom domains | ⚠️ Require explicit `--allow-custom-domain` approval |

A custom-domain approval is a warning boundary, not a public-domain allowlist. Antra warns when an approved domain uses a public-looking TLD and never edits the hosts file before approval.

### CA Key Safety

- Private key stored at `~/.config/antra/ca-key.pem` with `0o600` permissions
- Never logged, never printed, never committed to git
- Never transmitted over IPC
- `antra clean` removes the local key after trust and hosts cleanup succeeds

### CA Versioning and Rotation

The root certificate is versioned by a marker file (`~/.config/antra/.ca-version`) and rotated when the on-disk CA predates the current version. Rotation is what let v0.5.0 replace a root that every Apple-stack client refused to parse; it will also fire when the CA's own validity window ends.

| Rule | Why |
|------|-----|
| A CA carries no `subjectAltName` | A `dNSName` must be a valid DNS name. `DNS:Antra Local CA` made Safari and every macOS system TLS tool reject the chain at parse time, even when it was installed and trusted. |
| Validity is bounded (800 days) | Apple caps TLS server certificates at 825 days, custom roots included. rcgen's 1975→4096 default is outside that window. |
| The replaced CA is kept in `retired-ca.pem` | Trust-store removal must be byte-exact, and an interrupted rotation has to converge on the next run. |
| The retired CA is removed only after the new one is trusted | Same order as a user-driven change: never leave the machine trusting a root that signs nothing. |
| Removal is never silent | The system store may need elevation; when removal fails the file is kept and the exact retry command is printed (`sudo antra trust --remove`). |

Leaf certificates are cached per hostname and regenerated inside a 45-day renewal window, so the bounded validity window does not turn into an expiry surprise.

`antra doctor` reports whether the CA passes strict X.509 validation, whether a superseded CA is still present in a trust store, and whether the running daemon is serving a CA other than the one on disk (a daemon holds its CA for its whole lifetime, so it must be restarted after a rotation).

### IPC Socket Safety

The daemon's control socket is a Unix socket at `socket_path()`.

- The socket file itself is `0o600`, owner read/write only
- Its parent directory is `0o700`, created before the bind and tightened on
  every start — not only when it is first created
- Under `sudo antra proxy start` the daemon now drops to the invoking user
  before it binds the socket (see *Port 443 and `sudo`*), so it owns all of
  this. For a daemon that stays root under `sudo` (an older release, or a
  `SUDO_UID` with no passwd entry), the socket and
  every directory component created for it are chowned to the invoking user
  so unprivileged `status`/`stop` can connect while `0600` still holds —
  enforced for the user instead of root
- The bind is the singleton claim: exactly one starter wins, the loser fails
  with `EADDRINUSE` and takes no state

**The directory is what makes the socket safe.** A socket file is created by
`bind(2)` at the process umask, so the daemon cannot make it `0o600` at the
moment it appears — the best it can do is tighten it immediately afterwards,
leaving a window at the umask. `connect` needs `x` on every component of the
path, so a socket that is briefly `0o755` inside a `0o700` directory is
unreachable to anyone else for the whole of that window. The directory is
therefore made private *before* the bind; tightening it afterwards would only
move the race. Both socket locations get this: the preferred directory
(`$XDG_RUNTIME_DIR/antra` or the data-local `antra` dir) and the fallback
below.

The chown is not optional alongside the tightening. A root-owned `0o700`
directory is unsearchable for the user who ran `sudo`, which would lock the
unprivileged CLI out of its own daemon. Every component the daemon *created*
is chowned, not just the leaf — tightening the leaf alone would leave an
unsearchable root-owned ancestor above it and break the same flow. A
directory that belongs to neither the process's euid nor the invoking user
is left untouched and logged rather than silently trusted or reowned.

**Path length.** A Unix socket path is capped at 104 bytes (`sun_path`). The
preferred location is `$XDG_RUNTIME_DIR` (Linux) or the data-local dir
(macOS: `$HOME/Library/Application Support`). On macOS that second option
carries 50 bytes of fixed overhead, so any home directory longer than ~54
characters — a long username, or a CI runner's `mktemp -d` — would produce a
path that cannot be bound, failing with the bare
`path must be shorter than SUN_LEN`. When the derived path does not fit,
Antra falls back to `/tmp/antra-<uid>/<fnv1a-of-original-path>/d.sock`:
short enough to bind, still namespaced per user, still distinct per home so
two daemons cannot collide on the single short name. Every component of that
`/tmp` tree is created `0o700` (previously `0o755` from `create_dir_all`,
which let any local user list the per-home hashes) and chowned to the invoking
user.

### Hosts File Safety

- Only modify entries within `# BEGIN ANTRA MANAGED HOSTS` block
- Never overwrite unrelated entries
- Writes use a temporary file and replacement path
- Unix replacement is atomic; Windows replacement is best-effort
- `antra clean` verifies the managed block before and after removal

### Process Safety

- Child processes run in a separate process group
- Signal forwarding is explicit, not broadcast
- On cleanup, verify child is actually dead before removing route
- No `kill -9` unless grace period (5 seconds) expired

### Port 443 and `sudo` (ROADMAP C27)

On macOS and Linux only root may bind ports below 1024 on loopback (macOS
lifts the rule only for `0.0.0.0`, which Antra never binds). A URL without a
port therefore needs `sudo` once.

- **Asked, never assumed.** The first daemon start in a terminal asks
  `Use port 443? [Y/n]` and saves the answer in `config.toml`
  (`use_port_443`). No terminal, no question: a saved yes runs `sudo -n`,
  which fails rather than waits for a password. The offer is made only when
  binding 443 is *forbidden* (`EACCES`) — not when it is taken, when the user
  set `ANTRA_PORT`/`ANTRA_HTTP_PORT`, or when `sudo` is not on `PATH`.
- **Root only long enough to open the ports.** Under `sudo`, the daemon binds
  443 and 80 on loopback, then calls `initgroups`, `setgid` and `setuid` to
  the user named by `SUDO_UID`/`SUDO_GID`, and refuses to run if `setuid(0)`
  still succeeds afterwards. All of it happens before the tokio runtime
  exists, so no thread ever runs as root, and the CA, certificates, socket and
  pid file are created by the user. The short-lived `proxy start` launcher
  still opens the log as root and chowns it, as it did before.
- **The user's paths, not root's.** `sudo` on Linux resets `HOME` to root's
  and drops `XDG_RUNTIME_DIR`, which put a `sudo antra proxy start` daemon's
  socket under `/root`, invisible to the user's CLI. Under `sudo`, `antra
  proxy` replaces a `HOME` or `XDG_RUNTIME_DIR` that is root's with the
  invoking user's (`platform::sudo::paths_to_adopt`); a value passed on
  purpose is kept. The first-run offer passes the CLI's own values through
  `sudo -- /usr/bin/env …`, so both sides resolve exactly the same paths.
- **No idle exit.** A daemon started this way never idles out, since starting
  it again would mean another password prompt. `antra proxy stop` stops it.
- Only a daemon spawned by a `proxy start` launcher under `sudo` drops (the
  launcher marks it with `ANTRA_DROP_TO_USER`). Plain root (a container, a
  root login) and a daemon auto-started by a root CLI (`sudo antra run`) are
  unchanged: they stay root, in the same paths as the CLI that started them.

## Trust Store Modifications

Installing a custom root CA is classified as **MITRE ATT&CK T1553.004** (Subvert Trust Controls: Install Root Certificate). Antra must:

1. **Always prompt** before modifying the system trust store
2. **Explain** what will happen and why
3. **Provide** a way to undo (`antra trust --remove` or `antra clean`)
4. **Never** install silently or without consent

`antra run` and `antra dev` show a first-run `[Y/n]` prompt; Enter accepts. `--yes` is an explicit non-interactive opt-in, and `--no-trust-prompt` skips the flow for that invocation. `antra clean` removes the exact current CA from applicable system and user trust stores before deleting local state.

### Platform Behavior

| Platform | Consent Required | Notes |
|----------|-----------------|-------|
| macOS (Big Sur+) | GUI authentication dialog | Root alone is insufficient |
| macOS (pre-Big Sur) | Root access suffices | Headless install possible |
| Windows (user store) | Confirmation dialog | Can suppress with `--yes` |
| Windows (machine store) | Admin elevation | Triggers security software alerts |
| Linux | Root access suffices | No GUI dialog exists |

## Exclusions

- ❌ No cloud telemetry
- ❌ No network calls from core proxy
- ❌ No automatic system trust modification without consent
- ❌ No custom-domain hosts changes without explicit approval
- ❌ No credential logging
- ❌ No private key exposure
