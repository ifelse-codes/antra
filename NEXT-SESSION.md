# NEXT-SESSION.md — Instructions for the next session

> The user says **"start next session"** → read this file and execute the plan below.
> One phase remains: (A) decide the `e2e_all_features.sh` residuals, (B) wire the
> shell e2e suites into CI.

Repo: `main` is the default branch; work in a feature branch. Root docs are
`AGENT.md` / `README.md` / `ROADMAP.md` / this file.

## What already landed (do not redo)

**Phase A is done.** ROADMAP cleanup rows C4, C5, C6 are `DONE`, each with the
evidence cited. C7 (the `sun_path` socket-path guard) and the shell-suite repair
landed with them.

**The suites are repaired.** All four had the same structural defect, plus
several independent ones. Fixed:

- `ANTRA_BIN` was relative (`./target/debug/antra`) while every test `cd`s into
  a scratch dir under `/tmp`. Now anchored to `BASH_SOURCE`. This one line took
  `e2e_all_features.sh` from 3/70 to 46/70.
- ~35 source-level `grep`s resolved `src/...` against the working directory,
  which by then was `/tmp`. Now use `$REPO_ROOT`.
- `setup()` was defined but never called in three suites, so `$TEST_DIR` and the
  results file were never created.
- `cleanup()` deleted `$RESULTS_FILE` — the run's own output. Only `setup()`
  clears the previous run's file now.
- `grep -q "$opt"` with `opt="--domain"`: grep parsed the pattern as its own
  flag and failed, so all 7 `run` option assertions failed unconditionally.
  Fixed with `grep -qF -- "$opt"`.
- `grep -q "Continuous Port Sync" roadmap.md | grep -q "DONE"`: the first grep is
  `-q` and emits nothing, and the `else` branch called `log_pass` anyway. Fixed,
  and it now reads `ROADMAP.md` (the lower-case name only ever resolved on
  case-insensitive macOS).
- `test_select_resolver` asserted `count -eq 1` for `fn select_resolver`, which
  also matches the intended `select_resolver_for_registration`. The trailing
  `(` is what makes it 1.
- `pkill -f "node.*test"` matched any node process with "test" on its command
  line — a developer's Jest run, not just this suite's. Scoped to `$TEST_DIR`.
- `kill $(lsof -ti:4001)` killed whatever held 4001. Now tracks its own child PID.
- `test_prune_no_daemon` / `test_hosts_sync_no_daemon` assumed they ran first,
  but an earlier `antra run` auto-starts the daemon. They now stop it
  explicitly, so they test the precondition they are named for.

### Measured, on this machine, free ports via `ANTRA_PORT` / `ANTRA_HTTP_PORT`

| Suite | Before | After |
|-------|--------|-------|
| `e2e_all_features.sh` | 3 pass / 67 fail | 52 pass / 18 fail |
| `e2e_next_sprint.sh` | 9 pass / 46 fail | 55 pass / 0 fail |
| `e2e_portless_parity.sh` | 40 pass / 14 fail | 54 pass / 0 fail |
| `e2e_portless_simple.sh` | 34 pass / 1 fail | 34 pass / 1 fail |
| **total** | **86 / 128** | **195 / 19** |

## Phase A — Triage the 19 residual failures

Two suites are green. `e2e_all_features.sh` (18) and `e2e_portless_simple.sh`
(1) are not. Every one of the 19 is itemised below, and **none of them is a
product bug in the fix** — they are environment or stale-expectation problems:

- **Toolchain missing (12).** `yarn`, `bun`, `python` (only `python3` exists),
  `mix`, `elixir`, `php` are all absent here, so every suite that spawns one
  fails on `Failed to spawn`. Either install them on the runner or guard the
  assertions with `command -v` and report SKIP, not FAIL. SKIP is the better
  default: a missing interpreter is not a failed feature.
- **Port 8080 held (4).** An unrelated `ssh` listens on 8080 and 8443. Do not
  kill it. `cargo run command used` / `Default port 8080 for axum` /
  `go run command used` / `Default port 8080 for Go` fail because the run
  aborts at the port check before it ever prints the command it chose.
  `detect.rs:246` really does set `default_port: Some(8080)`, so the suite's
  expectation is right and the environment is wrong. These pass on a clean
  runner.
- **Needs root (1).** `.test domain resolution` — `--domain test-app.test` is a
  non-`.localhost` TLD, so it writes `/etc/hosts` and fails with
  `Permission denied writing /etc/hosts (needs sudo)`. Either run that
  assertion under sudo or drop the custom TLD and use a `.localhost` one.
- **Untriaged (1).** `Route added` in `e2e_portless_simple.sh`.
- **ROADMAP C9 (1).** `Port detected from --port flag` — a `vite --port 3001`
  dev script is ignored in favour of the framework default 5173. A real
  behaviour question, not a test bug. See C9.

## Phase B — Wire the suites into CI

1. Read `.github/workflows/ci.yml` and match its conventions.
2. Add a job that runs the four suites with `ANTRA_PORT` / `ANTRA_HTTP_PORT` on
   ports verified free on the runner, and a hermetic `HOME`.
3. **A short `HOME` matters.** `ipc/server.rs` now guards the `sun_path` limit,
   but a short `HOME` still keeps the tests fast and the paths readable. The
   Rust harness uses `/tmp/antra-e2e/<user>` for this reason.
4. Do not let a suite's leftover daemon leak into the next one. Run each suite
   with its own `HOME`, or stop the daemon between them.
5. Publish the suite output as a CI artifact — `$RESULTS_FILE` is now preserved
   for exactly this.
6. Report the pass/fail counts per suite in the PR body, with any SKIPs and why.

## Gates (all must pass before committing)

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (warnings are errors)
- Full `cargo test` in a **disposable HOME** (never the real HOME):

  ```bash
  # Capture the toolchain paths before redirecting HOME, or rustup cannot
  # find them and cargo fails with "could not choose a version of cargo".
  CARGO_HOME_REAL="${CARGO_HOME:-$HOME/.cargo}"
  RUSTUP_HOME_REAL="${RUSTUP_HOME:-$HOME/.rustup}"
  TEST_HOME=$(mktemp -d /tmp/antra-test.XXXXXX)
  HOME="$TEST_HOME" CARGO_HOME="$CARGO_HOME_REAL" RUSTUP_HOME="$RUSTUP_HOME_REAL" \
    cargo test -- --test-threads=4
  rm -rf "$TEST_HOME"
  ```
- All four suites: `for f in tests/e2e_*.sh; do bash -n "$f"; done`

## Delivery

Commit on a feature branch, push, open a PR against `main` (CI runs macOS +
Ubuntu + Windows; wait for all checks green), merge. After merge, update the
"Current state" line in `AGENT.md` and mark any ROADMAP row it lands.
