# Trust-friction verification — 2026-09-07 10:28 IST (toward 0.2.6)

Goal: kill every trust friction so setup ease goes 8 → 9+. All checks below
run live on macOS arm64, unprivileged (no sudo), against the working tree
(`cargo build` debug binary for Rust changes, live site for installer/page).

## What changed

- `src/trust.rs`: new `is_trusted_for_https()` (system OR login-keychain);
  `install_ca_noninteractive()` on macOS goes user-level-first and never
  touches the system store (no sudo/GUI prompts from automatic paths);
  interactive `install_ca()` on macOS non-root goes straight to a
  login-keychain prompt defaulting to yes (`install_ca_user_level_prompted`).
- `src/cli/run.rs`: first-run check uses `is_trusted_for_https()` (no more
  re-prompt when user-level trust exists); all hints say
  `antra trust --user-level` on macOS, incl. non-interactive skip.
- `src/cli/doctor.rs`: macOS trust fix is now the single executable command
  `antra trust --user-level`, so `Auto-fix all issues? [y]` works one-keypress.
- `install.sh` + `landing/install.sh`: macOS trust copy is login-keychain /
  no-sudo throughout; TTY-yes installs via `trust --user-level`;
  non-TTY hint points at `--user-level`.
- `landing/index.html`: Real-HTTPS card promises no-sudo; hero + No-ports
  notes already live from the prior round.
- Version bumped to 0.2.6 (Cargo.toml/lock, README badge).

## Verification (all live, all pass)

| # | Check | Result |
|---|-------|--------|
| 1 | `trust --status` while trusted | ✅ user-level green |
| 2 | `trust --user-level` while trusted | ✅ "already trusted", still exactly 1 keychain entry (idempotent) |
| 3 | Delete keychain cert → `--status` | ✅ `✗ CA is NOT trusted`, points at `--user-level` |
| 4 | Same state → `doctor` | ✅ `• CA not trusted (no warning-free HTTPS) → antra trust --user-level`, counted in 4 errors |
| 5 | `echo "" \| trust` (default-yes, no sudo) | ✅ installed to login keychain, exit 0 |
| 6 | Fresh trust → `proxy start` + `alias` + `curl -s` (full verification, no `-k`) | ✅ `nine-plus`, exit 0 |
| 7 | Installer TTY-yes under real pty (stub binary) | ✅ invoked `trust --user-level`, success line |
| 8 | Installer non-TTY (`< /dev/null`) | ✅ skip + `Run 'antra trust --user-level' later` |
| 9 | Live piped installer + `ANTRA_VERSION` pin forms + garbage pin | ✅ (prior round; redeployed since, re-fetched live copy confirmed) |
| 10 | `cargo test --offline` full suite | ✅ all suites ok, 0 failures |
| 11 | `run` non-interactive hint (debug binary) | ✅ `Run antra trust --user-level…`, route cleaned up |
| 12 | Live site re-fetch | ✅ keychain copy + no-sudo HTTPS card live |

## Score

- **Setup ease: 8 → 9.** Fresh macOS path is now: install → one `Y` (no sudo)
  → `run` → real HTTPS URL. No admin prompt, no elevation error, no re-prompt,
  no version surprise (pin), no 443 surprise (disclosed). Remaining point is
  structural: 443 itself still needs root by OS design (fallback `:8443`
  disclosed upfront), and Linux still needs sudo for trust (no user-level
  equivalent — documented in `trust --status`/doctor hints).
- Delivers-on-promise stays 9 (unchanged behavior, less friction to reach it).

## To ship

- Commit + `git tag v0.2.6 && git push origin main v0.2.6` → CI builds 5
  artifacts → publish the draft release → update `Formula/antra.rb` +
  installer pin example to v0.2.6. Landing is already redeployed (installer
  `--user-level` path works with the 0.2.5 binary too).
- State left clean: test routes removed, daemon stopped, user-level CA trust
  in place, pre-existing processes untouched.
