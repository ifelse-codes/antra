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
