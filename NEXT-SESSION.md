# NEXT-SESSION.md — Instructions for the next session

> Written to be read cold, by any tool. It does not assume you were present for
> the previous conversation.
>
>
> **Direction (maintainer, 2026-09-30): make Antra "good to market", then
> release once, then GTM.** `v0.6.3` is released; C17 is on `main`,
> unreleased. The maintainer's calls, not to be re-litigated:
> - **No release** until the launch-readiness fixes below are in; C17 rides
>   along with them.
> - **No GitHub Actions upgrade** for now (the Node 20 deprecation warnings
>   on `actions/checkout@v4` etc.) — revisit only if a workflow breaks.
> - **GTM starts only** once everything on the plate is done.
> - **Roadmap features** (LAN, monorepo, Tailscale/ngrok, …) only after
>   launch, and only on customer demand.
>
> **Start with "Still owed" — two decisions there are waiting on the
> maintainer.**

Repo: `main` is the default branch; work in a feature branch.

## Orientation — 60 seconds

**Antra** is a native macOS/Linux/Windows CLI that gives a local dev server a
stable HTTPS domain, so you stop memorising ports.

    antra run --domain myapp.localhost -- pnpm dev
    # → https://myapp.localhost

It runs a background daemon holding a local CA and a TLS proxy, injects
`PORT` and `NODE_EXTRA_CA_CERTS` into your child process, and routes
`*.localhost` subdomains at whatever port each app happens to use. The promise
is one command, a real HTTPS URL, your process unchanged. Rust, no runtime
dependencies. The competitive set is `portless` and ngrok-style tunnels; the
difference is Antra never leaves your machine.

**Read these, in order, before touching anything:**

1. `AGENT.md` — architecture, phase history, and a section headed *do not redo
   blindly* recording the ways this codebase has lied to itself. That section is
   the highest-value thing in the repo.
2. `ROADMAP.md` — what is done (C1–C16 all DONE).
3. `README.md` — the user-facing description and install paths.
4. `docs/security.md` — CA versioning, rotation, and the IPC socket rules. Read
   it before changing anything under `src/certs/` or `src/cli/service.rs`.

**Sanity check that you have a healthy checkout:**

    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings     # warnings are errors
    ./target/debug/antra --version                 # 0.6.1

Full test commands, including the two traps that cost real time, are under
**Gates** at the bottom of this file.

## Where things stand

`v0.6.3` is published, the landing site serves it, and both install paths
were checked on GitHub's macOS and Linux runners (**Release Check**): Homebrew
and `curl | bash`, latest and pinned, all report `antra 0.6.3`. v0.6.2 shipped
the Linux fixes (C14, C15); v0.6.3 the macOS service fix (C16), which the
**Service (macOS)** workflow now checks on a real Mac on every change to
`service.rs`. Release mechanics are in the v0.6.2 and v0.6.3 sections of
`AGENT.md`.

**Homebrew was broken for new users until today.** The tap repo
(`ifelse-codes/homebrew-antra`) sat on 0.2.3 from v0.3.0 through v0.6.1, since
releases only updated this repo's copy of the formula. Fixed with the v0.6.2
release; **Releasing** step 5 now names the tap.

The four shell e2e suites are green and **all four run in CI** on macOS and
Ubuntu (206 passing / 0 failing / 9 skipped when last counted, at v0.6.1). The
Rust suite is at 421 passing, and since C15 it passes in full in the IPv6-less
claude.ai cloud container too.
They were at 86 passing / 128 failing before this work, and the four suites had
never run in CI at all.

**Since v0.6.1:** `antra service install` on Linux was verified broken against
a real `systemd --user` manager, then fixed (ROADMAP C14). It had two bugs, not
one: the unit sat outside systemd's search path, *and* `antra proxy start`
forks and exits, so systemd killed the daemon and restarted it every 5 s. The
fix and the lessons are in the v0.6.2 section of `AGENT.md`;
`tests/manual_service_linux.sh` re-runs the 28-check verification.

**C13 is answered too:** pnpm inference was never wrong. The check waited for
the `pnpm run dev` spawn line, which needs pnpm installed — so the runners
almost certainly lack pnpm (not read from a CI log; the log host is blocked
from the cloud container). It now checks the inference with or without pnpm.

**And C15:** on a host with no IPv6 the daemon refused to start, blaming
"port in use" on free ports. It now listens on IPv4 alone there, and only
there. The claude.ai cloud container is such a host, so the whole test suite
now passes in it with the real binary.

## Still owed

Ordered by how many users they affect, not by how interesting they are.

**The goal is launch readiness:** a stranger installs Antra, runs an app and
gets a real HTTPS page with no browser warning, on macOS, Linux and Windows.
A 15-minute probe on 2026-09-30 in the (Linux) cloud container found three
real problems, each reproduced there:

| Item | Blast radius | What it takes |
|---|---|---|
| **Linux: Chrome warns after `antra trust`** | Every Linux user of Chrome, and very likely Firefox — the product's headline promise | `antra trust` installs the CA into the system store (`/usr/local/share/ca-certificates`), which `curl` uses but Chrome on Linux does not: Chrome reads its own NSS store, `~/.pki/nssdb`. Reproduced with Playwright's Chromium: `net::ERR_CERT_AUTHORITY_INVALID`; after `certutil -d sql:$HOME/.pki/nssdb -A -t "C,," -n "Antra Local CA" -i ca.pem`, the same page loads (200). Firefox keeps a per-profile NSS store and is untested. The fix is what mkcert does — also add the CA to the NSS stores via `certutil` (`libnss3-tools`), always behind the consent prompt — and `trust --remove` must undo it. **Needs the maintainer's OK**: Phase 6's exclusions say "No Firefox NSS store modification". |
| **`antra trust --remove --yes` ignores `--yes`** | Anyone uninstalling by script | It prints "Remove CA from system trust store? [y/N]", reads no answer without a TTY, and ends "Skipped. CA remains trusted." Install honours `--yes`; remove does not. |
| **The installer prints raw `\033[…m`** | Every new user, on first contact | `install.sh` defines its colours as `'\033[1m'` literals and prints 10 lines with plain `echo` (lines ~221–337: "Trusting the CA", "Quick start", "NEXT STEPS"…), which does not interpret them. First reported in `tests/user-test-2026-09-07-1430.md`, never fixed. Keep `landing/install.sh` identical; merging deploys it. |
| **A1 — automated browser check** (proposed, not started) | Proves or disproves the headline promise per browser/OS | A workflow on GitHub's macOS and Linux runners: `antra trust`, then Chrome, Firefox and (macOS) Safari load an Antra URL with no certificate error; a Vite HMR edit reaches the page; Ctrl+C removes the route with no orphans. Covers `docs/mvp.md`'s definition of done without a person. **Waiting on the maintainer's OK.** |
| **A2 — fresh "stranger" test of the current release** (proposed, not started) | Finds what the probe did not | The last one was `tests/user-test-2026-09-07-1430.md`, on v0.2.8. Same format: website → install → first run → core usage → error paths. **Waiting on the maintainer's OK.** |
| **Then: one release** | — | Carries the fixes above plus C17. Follow **Releasing** below. Then GTM. |

To reproduce the Chrome finding in the cloud container: Chromium is at
`/opt/pw-browsers`, Playwright is global (`NODE_PATH=/opt/node22/lib/node_modules
node script.js`), and `apt-get install -y libnss3-tools` provides `certutil`.
Undo `antra trust` by hand afterwards if `--remove --yes` is still broken
(`rm /usr/local/share/ca-certificates/Antra-Local-CA-*.crt &&
update-ca-certificates --fresh`).

**Deliberately not planned** (maintainer's call, 2026-09-30): the manual
Safari + Firefox pass on `docs/mvp.md`. It stays low value while the TLS half
is machine-checked in CI (`tests/e2e_securetransport.rs` asks Apple's own
stack via `/usr/bin/curl`). Do not re-add it to "Still owed" unprompted.

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

Every step runs on GitHub; none needs a local machine. The workflows are
started from the **Actions** tab (*Run workflow*), or by an agent through the
GitHub API.

1. Bump the version (`Cargo.toml`, `Cargo.lock`, `README.md` badge + status
   line) and write `docs/releases/v<version>.md`. Merge.
2. Push the tag `v<version>` on `main`. `release.yml` cross-compiles five
   targets into a **draft** release. A cloud agent session cannot push tags
   (its git proxy answers 403), so a person pushes this one.
3. Publish the draft, then confirm it is no longer a draft — during v0.6.3
   the first publish did not take. Then run **Release Notes** with the tag: it replaces
   GitHub's generated PR list with `docs/releases/<tag>.md` and reads it back.
4. Read each `sha256` from the release's own `.sha256` assets — do not hash
   locally, so a typo cannot creep into the formula. Check size alongside
   hash: a truncated transfer looks exactly like a corrupted release, and one
   did during v0.6.1.
5. Update `Formula/antra.rb` with the new version and those hashes, **and copy
   the same file to the tap repo, `ifelse-codes/homebrew-antra`**. That repo is
   what `brew install ifelse-codes/antra/antra` reads; the file in this repo is
   not. The tap sat on 0.2.3 from v0.3.0 through v0.6.1 because only this copy
   was updated, so Homebrew users got a build without the v0.5.0 CA fix.
   Update the pin examples in `install.sh`, `landing/install.sh` and
   `landing/index.html` in the same PR.
6. Merging a change under `landing/` runs **Deploy Landing**, which deploys a
   *production* deployment and then checks the live domain serves that commit
   (`.github/scripts/check-landing.sh`). It replaces the manual
   `wrangler pages deploy`, which from a feature branch created a *branch*
   deployment and silently left production on the old files. It needs the
   repository secrets `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`.
7. Run **Release Check** with the version: Homebrew and the `curl` installer
   (latest and pinned) on macOS and Linux runners, plus the live site. It is
   the only Homebrew check that needs no Mac at hand.

## Delivery

Commit on a feature branch, push, open a PR against `main`. CI runs macOS +
Ubuntu + Windows plus a ~15-minute e2e job; wait for all checks green, then
merge. After merge, update the "Current state" line in `AGENT.md` and flip any
ROADMAP row it lands.
