# C28 — `sudo antra trust` installed the wrong CA

**Status:** fixed on branch `fix/sudo-trust-friction`, verified live on Ubuntu 24.04 (x86_64).
**Scope:** P0 trust-under-sudo fix + P1 doctor NSS hint. No behavior change on macOS/Windows.
**Decision respected:** C19 stands — Antra never writes the NSS store itself.

## What was found

A fresh install from `https://antra.iifelse.com/install.sh` (byte-identical to
repo `install.sh`, v0.6.6 binary, new user) dead-ends on Linux trust:

1. `antra trust --yes` fails: `Could not install CA automatically. Try: sudo antra trust`
2. `sudo antra trust --yes` **succeeds** — but `trust --status` still reports
   `CA is NOT trusted`, and plain `curl` fails with `000`.
3. There is no documented path out; the loop repeats forever.

Measured fingerprints proved it: the installed system-store CA (serial
`310A…`) was neither the user's CA (`0DB0…`) — `sudo` resets `HOME` to
`/root`, so trust minted a **second CA under `/root`** and installed that,
while the user's daemon kept serving the user's CA. Same trap class C27
fixed for `proxy start`, whose `adopt_invoking_user_paths` the trust path
never called (`src/cli/proxy.rs` was the only caller).

Second finding, same session: even with system trust green, Chrome/Firefox on
Linux still warn (C19), and nothing where the user looks says so.

## What was solved

- **P0:** `sudo antra trust` / `sudo antra trust --remove [--yes]` now act on
  the invoking user's CA. `src/cli/trust.rs::execute` adopts the user's paths
  under sudo before any thread exists (mirroring `proxy start`), and
  `trust::hand_back_config_dirs` chowns `~/.config/antra/` + `certs/` back —
  CA files were already handed back by `atomic_write`; the directories were
  not, which would have locked the next unprivileged run out of its own CA.
  The hand-back only runs under `sudo` (a normal run already owns its dirs and
  must not gain a new way to fail there), and the cert-cache marker
  (`.leaf-version`) is now written through `atomic_write` too, so a
  root-written marker is handed back and replaced by rename rather than left
  root-owned for a later rotation to trip over.
- **P1:** `doctor` on Linux now reports the NSS gap at the point of success:
  `✓ CA trusted by Chrome/Firefox (NSS)`, or a copy-paste
  `mkdir -p ~/.pki/nssdb && certutil …` line. Informational only — no
  issues/warnings push, no exit-code change. The `mkdir` is load-bearing:
  fresh machines have no nssdb and `certutil` fails with
  `SEC_ERROR_BAD_DATABASE` (measured).

## How it was verified

- Bare `sudo antra trust --yes` (real HOME) → status trusted, system `curl`
  with no `--cacert` → **200**; config dirs stay user-owned.
- Bare `sudo trust --remove --yes` → absent, 0 store files, `curl` → 000.
- Doctor's NSS command run verbatim → doctor flips to NSS green.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (Linux +
  `x86_64-pc-windows-gnu`), `tests/installer_output.sh` 8/0/0, full
  `cargo test`: **521 pass / 1 fail** — the single failure is
  `upstream_dual_stack`, environmental (this VPS maps `localhost` to
  127.0.0.1 only, so `::1` is never resolved; fails identically without this
  change).
- New unit test pins the NSS command wording; mutation-checked red→green.

## Limits and leftovers

- A hermetic `HOME` under bare `sudo` is unfixable by design (sudo destroys
  the information; adoption falls back to the passwd home). Pass `HOME`
  through explicitly (`sudo env HOME=…`) in that setup.
- `trust --remove` does not touch the NSS store (per C19); a manually-added
  NSS entry must be removed with `certutil -D` by the user.
- Friction remaining for a later pass: two elevation prompts on first run
  (CA + port 443), `~/.local/bin` PATH onboarding, and the C19 exclusion
  itself (needs a maintainer call to lift).
