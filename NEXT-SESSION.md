# NEXT-SESSION.md — Instructions for the next session

> The user says **"start next session"** → read this file and execute the plan below.
> This is scheduled work; it is not a question. Two phases: (A) fix stale ROADMAP
> cleanup statuses, (B) wire the shell e2e suites into CI.

Repo: `/Users/suman/playground/antra` — branch `main`, working tree clean,
origin/main in sync. Root docs are `AGENT.md` / `README.md` / `ROADMAP.md` /
this file.

## Phase A — C4/C5/C6 are already shipped; fix the ROADMAP (small)

The ROADMAP.md cleanup table lists C4 (`Remove unused proxy/server.rs`), C5
(`TTY check in doctor`), C6 (`Fix stale socket detection`) as NOW / NEXT, but
the code is **already done** (verified on main, do not redo them):

- **C4** — `src/proxy/server.rs` is absent from HEAD (`git cat-file -e HEAD:src/proxy/server.rs` fails) and there are no dangling `proxy::server` references.
- **C5** — `src/cli/doctor.rs` guards the stdin auto-fix read with `libc::isatty(libc::STDIN_FILENO) != 0` (offer auto-fix only when stdin is a TTY).
- **C6** — `src/ipc/client.rs::is_daemon_running()` connects to the socket (`UnixStream::connect`) on Unix and probes the named pipe (with retry on Windows error 231) instead of stat-ing the path; `remove_stale_socket()` is the recovery helper.

**Change:** update `ROADMAP.md` cleanup rows C4, C5, C6 → `DONE`, each with a
one-line note citing the evidence above. This is the only change needed for
the C4/C5/C6 item.

## Phase B — Wire the shell e2e suites into CI (the real work)

Prior ROADMAP #21 (env vars) shipped: the auto-started daemon now inherits
`ANTRA_PORT` / `ANTRA_HTTP_PORT`, so the port-override blocker is gone.

Four shell suites exist and were **never** wired into CI:

- `tests/e2e_all_features.sh`
- `tests/e2e_next_sprint.sh`
- `tests/e2e_portless_parity.sh`
- `tests/e2e_portless_simple.sh`

The harness bug (`log_pass` used `((pass_count++))`, which exits 1 when the
counter is 0 → aborts under `set -e`) is already fixed in three of the four
(`e2e_portless_simple.sh` has no `set -e`).

On this dev machine the daemon cannot bind 443 (no root) and 8443/8080 are
held by an unrelated `ssh` — **do not kill unrelated processes to free ports.
Do not rely on 8443 being free locally.** The suites must run on a CI runner
against ports that are known-free there, configured via `ANTRA_PORT` /
`ANTRA_HTTP_PORT` (e.g. 8443 + 18080).

**Plan:**
1. Read the four suites and the existing CI workflow (`.github/workflows/`) to
   match conventions (matrix, hermetic HOME, artifact/cache setup).
2. Add a CI job that runs the suites against env-configured free ports.
   Confirm each suite exports/sets `ANTRA_PORT` / `ANTRA_HTTP_PORT` before
   invoking the daemon; add it if not.
3. Run the suites locally against a **free** port to triage failures before
   submitting (expect a nonzero pass/fail count; the ~9/46 baseline was with
   the blocked ports — improve it). Fix genuine suite bugs, not the
   environment.
4. Measure and report the pass/fail counts per suite in the PR body.

## Gates (all must pass before committing)

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (warnings are errors)
- Full `cargo test` in a **disposable HOME** (never the real HOME):

  ```bash
  TEST_HOME=$(mktemp -d /tmp/antra-test.XXXXXX)
  HOME="$TEST_HOME" CARGO_HOME=/Users/suman/.cargo RUSTUP_HOME=/Users/suman/.rustup cargo test -- --test-threads=4
  rm -rf "$TEST_HOME"
  ```

## Delivery

Commit on a feature branch, push, open a PR against `main` (CI runs macOS +
Ubuntu + Windows; wait for all checks green), merge. Follow the existing
commit convention in the repo history. After merge, update the "Current state"
line in `AGENT.md` to reflect the wiring, and mark any ROADMAP row it lands.
