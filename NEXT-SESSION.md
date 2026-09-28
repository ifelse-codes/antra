# NEXT-SESSION.md — Instructions for the next session

> The user says **"start next session"** → read this file and execute the plan below.
> One phase remains: wire the shell e2e suites into CI. They are green and ready
> (195 pass / 0 fail / 11 skip, all exiting 0).

Repo: `main` is the default branch; work in a feature branch. Root docs are
`AGENT.md` / `README.md` / `ROADMAP.md` / this file.

## What already landed (do not redo)

**Phase A is done.** ROADMAP cleanup rows C4, C5, C6 are `DONE`, each with the
evidence cited. C7 (the `sun_path` socket-path guard), the shell-suite repair,
and the SKIP/port guards all landed. C9 and C10 remain filed and unfixed.

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
| `e2e_all_features.sh` | 3 pass / 67 fail | 49 pass / 0 fail / 11 skip |
| `e2e_next_sprint.sh` | 9 pass / 46 fail | 55 / 0 / 0 |
| `e2e_portless_parity.sh` | 40 pass / 14 fail | 56 / 0 / 0 |
| `e2e_portless_simple.sh` | 34 pass / 1 fail | 35 / 0 / 0 |
| **total** | **86 / 128** | **195 / 0 / 11** |

All four exit 0.

## Phase A — DONE. The suites are green.

The last 19 are cleared and the fixes are worth remembering:

- **Missing toolchain → SKIP, not FAIL.** 11 tests are gated on `command -v`
  for `yarn`, `bun`, `python`, `mix`, `php`. A missing interpreter is not a
  broken feature, and a suite that goes red for it teaches people to ignore
  it. Skips are counted and printed in the summary so a green run never claims
  more than it checked. `1 .test domain resolution` also SKIPs, because
  resolving a non-`.localhost` TLD writes `/etc/hosts` and needs root.
- **Framework-default port tests assert the choice, not the bind.** They
  failed because 8080 was occupied, so the run stopped before printing the
  route. Both paths name the port, so `grep 8080` is the honest
  environment-independent assertion; the command assertion moved to a second
  run forced onto a probed-free port.
- **No more hardcoded ports.** `free_port` probes with bash's `/dev/tcp`. 3001
  turned out to be held by an unrelated desktop app on this machine — the
  Agent Orchestrator — which is exactly the failure mode a literal port invites.
- **ROADMAP C9 is a characterisation test now.** `vite --port 3001` inside a
  `package.json` script is genuinely ignored, because `detect_port_from_command`
  only sees the argv Antra execs (`npm run dev`) and the flag stays inside the
  script body. The test asserts the current behaviour on purpose, so fixing
  C9 makes it fail loudly instead of silently changing what the suite claims.
- **A suite "end-to-end" test that could not fail.** `test_e2e_real_server`
  started its server with `timeout 5` — GNU coreutils, absent on macOS — so
  the command died instantly, `kill -0` saw a dead pid, and the `else` branch
  logged a pass. It never ran a server. It now starts a real one, registers a
  route, and fetches back through the proxy over HTTPS.

  Verified the replacement can fail: changing the served body to `WRONG BODY`
  produced exactly one failure, `Proxy forwards requests correctly over HTTPS
  (got: WRONG BODY)`, and nothing else. **Check that a new assertion can fail
  before trusting a green suite.**

Also fixed while clearing these: `e2e_portless_parity.sh` had two more
`else log_pass` branches that reported passes they had not earned, and both
suites asserted on `Added route`, a line a previous session removed from
`src/cli/add.rs` as redundant.

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
