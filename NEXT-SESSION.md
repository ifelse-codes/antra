# NEXT-SESSION.md — Instructions for the next session

> The user says **"start next session"** → read this file and execute the plan below.
>
> **Direction: `v0.6.1` is released and the roadmap's engineering items are
> closed. No new features.** Work should be GTM, plus the three items in
> "Still owed" below, which are all small.

Repo: `main` is the default branch; work in a feature branch. Root docs are
`AGENT.md` / `README.md` / `ROADMAP.md` / this file.

## Where things stand

`v0.6.1` is published, live on the landing site, and installable via both Homebrew
and `curl | bash`. Release mechanics and post-release verification are recorded
in the v0.6.1 section of `AGENT.md`.

The four shell e2e suites are green and **all four now run in CI** on macOS and
Ubuntu: 206 passing / 0 failing / 9 skipped. The Rust suite is at 406 passing.
They went from 86 passing / 128 failing at the start of this work.

ROADMAP C1–C12 are done. C8 is now `DONE` rather than `DONE (2 of 4)`.

## Still owed — all small

| Item | What it is | Why it matters |
|---|---|---|
| **C14** | `antra service install` is likely broken on Linux | The unit is written to `~/.config/antra/systemd/user/`, which `systemctl --user` does not search, then `systemctl --user enable` runs with no `daemon-reload` or `--user link`. ROADMAP #6 claims it shipped — verify on a real Linux box before a user reports it. Fixing it means writing to systemd's search path or linking the unit, plus deciding what happens to units already on disk. |
| **C13** | `pnpm` inference differs on GitHub runners | Now gated, so it cannot fail the job, but the question is open: it fails on **both** runners while passing on a machine that has pnpm. Reproduce with `test_node_pnpm` and pnpm removed from `PATH`. |
| **Browser pass** | Safari + Firefox | `docs/mvp.md` still wants a human run. The Safari-critical half is machine-checked in CI (`tests/e2e_securetransport.rs`, which asks Apple's own TLS stack via `/usr/bin/curl`), but the browser itself is not. |

## Two things that will bite whoever touches the tests

Recorded in `AGENT.md` under "do not redo blindly", repeated here because they
are the most expensive lessons in this codebase:

- **A green suite proves nothing until you have checked that green is reachable
  by failure.** Five separate ways a check passed while proving nothing, one of
  which grepped for `"uvicorn"` — a string that also appears inside the
  `Failed to spawn 'uvicorn'` error it was meant to detect.
- **A toolchain gate is not a timeout, and a capped process has two children.**
  `antra dev` runs the project's dev command in the foreground, so a server
  command blocks forever. `run_antra_capped` caps it and kills only the dev
  command — the one that leads its own process group — leaving the daemon up.

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
  `HOME` is required for the C7 `sun_path` limit, and they need `cargo build`
  first:

  ```bash
  cargo build
  for f in tests/e2e_*.sh; do
    H="/tmp/ah-$(basename "$f" .sh)"; rm -rf "$H"; mkdir -p "$H"
    HOME="$H" ANTRA_PORT=18999 ANTRA_HTTP_PORT=18998 ANTRA_TIMEOUT=20 \
      bash "$f" || echo "FAILED: $f"
  done
  ```

  Expect roughly 20 minutes for all four. They are also wired into CI, so if CI
  is green you can lean on that instead of running them locally.

- Do **not** run `cargo test` in two worktrees at once without checking C11
  first. The harnesses are namespaced per worktree now, but a stale
  `/tmp/antra-e2e` from before that fix can still hold a crossed CA pair —
  `rm -rf /tmp/antra-e2e` if a certificate test fails in a way that reads like
  an ASN.1 bug.

## Releasing

Tag `v*` triggers `.github/workflows/release.yml`, which cross-compiles five
targets and creates a **draft** release. The draft is not published
automatically. Then:

1. Publish it with real release notes (`gh release edit <tag> --draft=false
   --title ... --notes-file ...`).
2. Read each `sha256` from the release's own `.sha256` assets — do not hash
   locally, so a typo cannot creep into the formula.
3. Update `Formula/antra.rb` with the new version and those hashes.
4. If `install.sh` or `landing/` changed, redeploy the landing site **as a
   production deployment**: from `landing/`, with an explicit `--branch main`.
   `wrangler pages deploy .` infers the branch from git, and from a feature
   branch it creates a *branch* deployment that silently leaves production on
   the old assets.
5. Verify against the **live domain**, not the deployment URL.
6. Check size alongside hash when verifying a download. A truncated transfer
   looks exactly like a corrupted release, and one did during v0.6.1.

## Delivery

Commit on a feature branch, push, open a PR against `main`. CI runs macOS +
Ubuntu + Windows plus a ~15-minute e2e job; wait for all checks green, then
merge. After merge, update the "Current state" line in `AGENT.md` and flip any
ROADMAP row it lands.
