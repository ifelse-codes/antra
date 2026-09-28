# NEXT-SESSION.md — Instructions for the next session

> The user says **"start next session"** → read this file and execute the plan below.
> One phase remains: get the two detection suites into CI (ROADMAP C12).

Repo: `main` is the default branch; work in a feature branch. Root docs are
`AGENT.md` / `README.md` / `ROADMAP.md` / this file.

## What already landed (do not redo)

Everything in ROADMAP C4 through C13 that is marked DONE. In one session:

- **C4-C6** verified shipped and marked DONE with the evidence.
- **C7** — `socket_path()` could exceed the 104-byte `sun_path` limit on any
  macOS home over ~54 chars, making the daemon unstartable with a bare
  `path must be shorter than SUN_LEN`. Now falls back to a short per-uid path.
- **C8** — an `E2E Shell Suites` CI job on macOS + Ubuntu, running
  `e2e_portless_parity.sh` and `e2e_portless_simple.sh`.
- **C9** — `antra dev` honours a port pinned in a `package.json` dev script.
  Precedence: CLI `--port` > dev-script pin > framework default > auto-assign.
- **C10** — `platform::ensure_private_dir` makes the socket's parent chain
  `0o700` *before* the bind, closing the bind-then-chmod window and the
  `0o755` fallback directory in one move.
- **C11** — both test harnesses namespace their `/tmp` paths per worktree.

The four shell suites went from 86 passing / 128 failing assertions to
**195 / 0 / 11 skipped**, all exiting 0. `e2e_all_features.sh` was then made
Linux-safe. Full Rust suite: **400 passing**.

## Phase A — Get the two detection suites into CI (C12)

`e2e_all_features.sh` and `e2e_next_sprint.sh` are excluded from the E2E job.
One of the two reasons is already fixed; the other is not.

**Reason (a) — SOLVED.** `e2e_all_features.sh` had 37 uncapped `antra dev`
calls, and one whose dev command is a server (`python -m http.server`) hung
the suite until the CI job's own timeout — 45 minutes. It was visible only on
Linux, because `python` is on the runner's PATH there so the toolchain gate
does not skip, while on a Mac `python` is absent and the test skips. All of
them are capped by `run_antra_capped` now. Verified locally by putting a
`python` → `python3` shim on PATH to reproduce the Linux condition:

```
python on PATH : 57 pass / 0 fail /  9 skip   exit 0   (622s, bounded)
macOS baseline : 49 pass / 0 fail / 11 skip   exit 0   (522s)
```

**Reason (b) — OPEN.** The two detection suites assert on `antra dev`'s spawn
line (`Started: <cmd>`), which is only printed after a successful spawn, so
they need a full toolchain on the runner. And `e2e_next_sprint.sh` has a
platform bug: its service-status assertion greps for `"not installed"`, but
Linux prints `"installed but not running"`.

Do this:

1. Fix the `e2e_next_sprint.sh` service assertion to accept both phrasings,
   or gate it on the platform.
2. Audit the remaining spawn-line assertions in `e2e_all_features.sh` the same
   way the `uvicorn` one was audited: an assertion that greps for a string which
   also appears in a `Failed to spawn '<x>'` error passes while proving nothing.
   Gate each on the binary it actually needs. Known good already: `cargo run`,
   `go run`, `Django manage.py runserver` — that last one is genuine because
   python exists, so antra prints the spawn line and the *child* then fails on
   a missing `manage.py`.
3. Add both suites to the E2E job, behind the same hermetic `HOME` and free-port
   setup. Report the SKIP count in the job output — a green run with 9 skips is
   a different claim from "every feature works".
4. Once they are in CI, re-check C13 (`pnpm command inferred correctly` fails
   on ubuntu only, unreproducible locally). If it still fails there, the
   detector is picking a different package manager than the lockfile implies.

## Phase B — The manual browser pass

`docs/mvp.md` still wants a human Safari + Firefox run. The Safari-critical
half is machine-checked in CI (`tests/e2e_securetransport.rs`, which asks
Apple's own TLS stack via `/usr/bin/curl`), but the browser itself is not.

## Gates (all must pass before you commit)

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (warnings are errors)
- Full `cargo test`. Capture the toolchain paths first, or rustup cannot find
  them once `HOME` is redirected:

  ```bash
  CARGO_HOME_REAL="${CARGO_HOME:-$HOME/.cargo}"
  RUSTUP_HOME_REAL="${RUSTUP_HOME:-$HOME/.rustup}"
  TH=$(mktemp -d /tmp/antra.XXXXXX)
  HOME="$TH" CARGO_HOME="$CARGO_HOME_REAL" RUSTUP_HOME="$RUSTUP_HOME_REAL" \
    cargo test -- --test-threads=4
  rm -rf "$TH"
  ```

- The shell suites, each with its own `HOME` and a distinct port pair. A short
  `HOME` is required for the C7 `sun_path` limit:

  ```bash
  for f in tests/e2e_*.sh; do
    H="/tmp/ah-$(basename "$f" .sh)"; rm -rf "$H"; mkdir -p "$H"
    HOME="$H" ANTRA_PORT=18999 ANTRA_HTTP_PORT=18998 ANTRA_TIMEOUT=20 \
      bash "$f" || echo "FAILED: $f"
  done
  ```

- Do **not** run `cargo test` in two worktrees at once without checking C11
  first. The harnesses are namespaced now, but a stale `/tmp/antra-e2e` from
  before the fix can still hold a crossed CA pair — `rm -rf /tmp/antra-e2e` if a
  certificate test fails in a way that reads like an ASN.1 bug.

## Delivery

Commit on a feature branch, push, open a PR against `main`. CI runs macOS +
Ubuntu + Windows plus the E2E job; wait for all checks green, then merge. After
merge, update the "Current state" line in `AGENT.md` and flip the ROADMAP rows
it lands.
