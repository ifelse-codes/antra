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

**Reason (b) — OPEN.** The two detection suites assert on `antra dev`'s
**spawn line** and on **registered routes**, neither of which is observable
unless the referenced toolchain is installed on the runner.

The rule, verified against the suite: **a test asserting only on *detection* is
safe ungated** — the Node framework tests pass on a lean runner because
`npm` is present and detection never needs the framework itself. **A test
asserting the chosen command or a route needs a `need` gate**, because the
spawn line is only printed after a successful spawn.

Ten tests are gated (`yarn`, `bun`, `python`×4, `mix`×2, `php`×2, and the
root-requiring `.test` case). Twenty-five are not, and these are the ones that
fail, with the assertions observed failing on GitHub runners:

| Test | asserts on detection | needs a gate |
|---|---|---|
| `node_pnpm` | — | `pnpm run dev` — **fails on both runners** |
| `go_gin`/`go_echo`/`go_fiber`/`go_chi` | `Go project detected` | `go run`, `Default port 8080` |
| `ruby_rails` | `Ruby Rails project detected` | `Rails server command used`, `Default port 3000` |
| `ruby_sinatra` | `Ruby Sinatra project detected` | `Default port 4567` |
| `ruby_generic` | — | `Rackup command used` |

Also `e2e_next_sprint.sh:322` greps for `"not installed"` but Linux prints
`"installed but not running"`. **That one is a product bug, not a
phrasing problem — the test is asserting the honest wording and should not be
changed.** `cli/service.rs:316-338` infers installed-vs-not from a single
`systemctl --user is-active antra-proxy` code: `active` → running, `inactive`
→ "installed but not running" plus `Run: systemctl --user start`, anything
else → "not installed" plus `Run: antra service install`. `is-active` does not
reliably separate an absent unit file from a stopped one across systemd
versions, and on the ubuntu runner it returns `inactive` for a service that
was never installed — so the user is told to `systemctl --user start` something
that does not exist, instead of `antra service install`. Distinguish the two
properly (`is-enabled`, or the presence of the unit file) and leave the test
alone.

Do this:

1. Gate the spawn-line assertions above on the binary each one needs.
2. Split the port assertions the way the axum and Go tests already do — assert
   the port *choice* from a bare run, the command from a second run on a
   probed-free port — so they stop depending on the toolchain being present.
3. Fix the service-status inference in `cli/service.rs` as described above.
   Do not loosen the assertion to match the wrong output.
4. Audit the rest of the spawn-line assertions the way the `uvicorn` one was
   audited: an assertion that greps for a string which also appears inside a
   `Failed to spawn '<x>'` error passes while proving nothing. Known good
   already: `cargo run` and `Django manage.py runserver` — the latter is
   genuine because python exists, so antra prints the spawn line and the
   *child* then fails on a missing `manage.py`.
5. Add both suites to the E2E job, behind the same hermetic `HOME` and
   free-port setup. Report the SKIP count in the job output — a green run with
   9 skips is a different claim from "every feature works".
6. Then chase C13, which is the one failure gating will not explain: `pnpm
   command inferred correctly` fails on **both** runners while passing on this
   machine, which has pnpm. Reproduce it by running `test_node_pnpm` with pnpm
   removed from PATH. If it passes without pnpm, it is a gating problem after
   all; if it still fails, the detector is picking a different package manager
   than the lockfile implies.

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
