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
- Under `sudo antra proxy start` with a user-owned `HOME`, the socket is
  chowned to the invoking user so unprivileged `status`/`stop` can connect
  while `0600` still holds — enforced for the user instead of root
- The bind is the singleton claim: exactly one starter wins, the loser fails
  with `EADDRINUSE` and takes no state

**Path length.** A Unix socket path is capped at 104 bytes (`sun_path`). The
preferred location is `$XDG_RUNTIME_DIR` (Linux) or the data-local dir
(macOS: `$HOME/Library/Application Support`). On macOS that second option
carries 50 bytes of fixed overhead, so any home directory longer than ~54
characters — a long username, or a CI runner's `mktemp -d` — would produce a
path that cannot be bound, failing with the bare
`path must be shorter than SUN_LEN`. When the derived path does not fit,
Antra falls back to `/tmp/antra-<uid>/<fnv1a-of-original-path>/d.sock`:
short enough to bind, still namespaced per user, still distinct per home so
two daemons cannot collide on the single short name.

Known gaps, tracked in ROADMAP C10: the socket is `bind`-ed before its mode is
tightened to `0o600`, so there is a brief window at the process umask; and the
`/tmp` fallback directory is created by `create_dir_all`, which yields `0o755`
rather than a private `0o700`.

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
