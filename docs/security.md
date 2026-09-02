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
| Known public domains (google.com, github.com, etc.) | ❌ Reject unless `--allow-public-domain` |
| Any other custom domain | ⚠️ Require explicit `--allow-custom-domain` flag |

### CA Key Safety

- Private key stored at `~/.config/antra/ca-key.pem` with `0o600` permissions
- Never logged, never printed, never committed to git
- Never transmitted over IPC
- `antra clean` securely deletes the key

### Hosts File Safety

- Only modify entries within `# BEGIN ANTRA MANAGED HOSTS` block
- Never overwrite unrelated entries
- Atomic write (write to temp file, then rename)
- Handle concurrent modification by re-reading before write

### Process Safety

- Child processes run in a separate process group
- Signal forwarding is explicit, not broadcast
- On cleanup, verify child is actually dead before removing route
- No `kill -9` unless grace period (5 seconds) expired

## Trust Store Modifications

Installing a custom root CA is classified as **MITRE ATT&CK T1553.004** (Subvert Trust Controls: Install Root Certificate). Antra must:

1. **Always prompt** before modifying the system trust store
2. **Explain** what will happen and why
3. **Provide** a way to undo (`antra trust --remove`)
4. **Never** install silently or without consent

### Platform Behavior

| Platform | Consent Required | Notes |
|----------|-----------------|-------|
| macOS (Big Sur+) | GUI authentication dialog | Root alone is insufficient |
| macOS (pre-Big Sur) | Root access suffices | Headless install possible |
| Windows (user store) | Confirmation dialog | Can suppress with `-f` |
| Windows (machine store) | Admin elevation | Triggers security software alerts |
| Linux | Root access suffices | No GUI dialog exists |

## Exclusions

- ❌ No cloud telemetry
- ❌ No network calls from core proxy
- ❌ No automatic system trust modification without consent
- ❌ No silent hosts file changes
- ❌ No credential logging
- ❌ No private key exposure
