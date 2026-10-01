# NEXT-SESSION.md — Instructions for the next session

> Written to be read cold, by any tool. It does not assume you were present for
> the previous conversation.
>
>
> **Direction (maintainer, 2026-09-30): make Antra "good to market", then
> release once, then GTM.** `v0.6.3` is released. The launch-readiness work
> below is **done on `main` and unreleased** (installer escapes, `trust
> --remove --yes`, A1, A2, C17). The maintainer's calls, not to be
> re-litigated:
> - **One release next**: v0.6.4, carrying all of it. Then GTM.
>   The code is **merged to `main`** (#49) and green; it is unreleased, so a
>   user running `curl | bash` today still gets v0.6.3 with the raw-escape bug.
> - **The Linux Chrome/Firefox certificate warning is accepted for now**
>   (decision, 2026-10-01): Phase 6's "No Firefox NSS store modification"
>   exclusion stands. Do not reopen it without being asked. The gap is
>   asserted on every run by the browser check, so it stays visible.
> - **No GitHub Actions upgrade** for now (the Node 20 deprecation warnings
>   on `actions/checkout@v4` etc.) — revisit only if a workflow breaks.
> - **The manual Safari + Firefox pass on `docs/mvp.md` is not planned** and
>   must not be re-added to "Still owed" unprompted. The browser check cannot
>   replace it on macOS: `antra trust` cannot install there at all, so that leg
>   only proves `curl --cacert`. Measured, not assumed — see the table below.
> - **Roadmap features** (LAN, monorepo, Tailscale/ngrok, …) only after
>   launch, and only on customer demand.
>
> **Start with "Still owed" — the next thing is v0.6.4, and the browser
> workflow's first CI run.**

Repo: `main` is the default branch; work in a feature branch.

**Status as of 2026-10-01:** the launch-readiness session is **merged** (#49,
`f3a957c`) and `main` is green — CI 9/9, Browsers 2/2, Deploy Landing deployed
the fixed installer and the live site serves it. Everything below is either
unreleased, open by decision, or Session 2 work.

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
    ./target/debug/antra --version                 # 0.6.3

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
The three problems below were found by a probe on 2026-09-30; two are now
**fixed on `main` and unreleased**, and the third is **open by decision**. A1
and A2 are **built**. v0.6.4 carries the lot.

| Item | Blast radius | What it takes |
|---|---|---|
| **Linux: Chrome warns after `antra trust`** | **OPEN, by decision.** Every Linux user of Chrome and Firefox — the product's headline promise | `antra trust` installs the CA into the system store (`/usr/local/share/ca-certificates`), which `curl` uses but Chrome and Firefox on Linux do not: they read `~/.pki/nssdb`. A first run on a Linux box shows `ERR_CERT_AUTHORITY_INVALID` in the two most common browsers while every machine-checked path says fine. The maintainer chose on 2026-10-01 to keep the Phase 6 exclusion ("No Firefox NSS store modification"), so this ships as a known gap. The fix, if ever wanted, is what mkcert does: also add the CA via `certutil -d sql:$HOME/.pki/nssdb -A -t "C,,"` (`libnss3-tools`), behind the existing consent prompt, undone by `trust --remove`. `.github/scripts/check-browsers.sh` asserts it on every run as an **expected** failure carrying that reason, so it stays visible; lifting the exclusion turns the same assertion into a real pass. |
| **`antra trust --remove --yes` ignores `--yes`** | **FIXED.** Anyone uninstalling by script | `src/cli/trust.rs` matched `--remove` before reading `--yes`, so the command always took the interactive path, read EOF, printed "Skipped. CA remains trusted." and exited 0. `remove_ca_noninteractive()` already existed and was correct — only `antra clean` called it. The routing is now a pure `action_for()` with five unit tests, three confirmed to fail against an inverted `if yes`. An end-to-end test was written and then **deleted**: in a hermetic home there is no `ca.pem`, so both paths exit 0 before the prompt and it passed against the broken code. |
| **The installer prints raw `\033[…m`** | **FIXED.** Every new user, on first contact | Ten lines in `install.sh` printed `${BOLD}` through plain `echo`, which does not interpret backslash escapes, so a new user saw the literal text `\033[1mTrusting the CA\033[0m` in the trust prompt and the quick-start block. Reported 2026-09-07, unfixed through four releases. A `say()` helper using `printf %b` now does what `info`/`ok`/`header` already did. `tests/installer_output.sh` covers it; its counters are `t_`-prefixed because `install.sh` defines its own `ok`, and an unprefixed helper here was being silently replaced by the installer's, printing green ticks while counting nothing. |
| **A1 — automated browser check** | **BUILT, GREEN AND VERIFYING** | `.github/workflows/browser.yml` + `.github/scripts/check-browsers.sh`, on push and PR, macOS + Ubuntu. The script stands up the daemon, an upstream and a route itself, and refuses to ask a browser anything until the route serves 200 over TLS. It needed six CI runs to be trustworthy — see the table below and ROADMAP C21 for how it was green while launching no browser at all. |
| **A2 — fresh "stranger" test** | **DONE** | `tests/user-test-2026-10-01.md`, against the released v0.6.3 binary on macOS. The product works: real HTTPS at a stable URL, a correct 301, a clean Ctrl+C, an honest `doctor`. Three new findings, filed below. |
| **Then: one release** | **TODO** | v0.6.4 carries the installer fix, the `trust --remove --yes` fix, C17 and the two new checks. Follow **Releasing** below. The notes **must** state the Linux Chrome/Firefox gap and the `certutil` workaround: the installer says "zero browser warnings — forever", and on Linux Chrome that is not yet true. |

### The browser check is green and actually verifying (2026-10-01)

Six CI runs were needed, and the sequence is worth reading — ROADMAP **C21** has
the detail. In order: a missing exec bit; `sudo` minting the CA under root's
home; `sudo -H` repeating that mistake explicitly; minting without sudo failing
*before* writing anything; `sudo -E` being refused because runners deny
`SETENV`, with a `|| true` swallowing the refusal; and finally `npm install -g`
leaving Playwright off node's module path, so the job went **green having
launched no browser at all**. That last one is the one to remember: the check
written to catch vacuous green was itself vacuously green.

It is now green for the right reasons, and here is what each leg actually
measured:

| Leg | Result | What it proves |
|---|---|---|
| `ubuntu-latest` | 5 pass / 0 fail / 3 expected-fail | Chrome `ERR_CERT_AUTHORITY_INVALID`, Firefox `SEC_ERROR_UNKNOWN_ISSUER` — **C19 reproduced automatically on a real runner**, attributed by cause. `curl --cacert` 200, so Antra's own TLS is sound |
| `macos-latest` | 4 pass / 0 fail / 4 expected-fail | The CA cannot be installed there, so every browser line says so instead of blaming C19. `curl --cacert` 200 |

`antra trust` on a macOS runner: `--user-level` returns *Failed to install to
user keychain*, and the privileged run cannot write the system keychain
without a GUI authorisation dialog. The same pair of failures reproduces on a
real Mac, so it is not a runner artefact — see **C20**.

**If you want a real macOS browser assertion**, it needs a trust store the
runner can write. Cheapest option: `security add-trusted-cert` into a temporary
keychain and point `HOME` at it. Not done here, and not needed for v0.6.4.

### From A2, not yet filed

| Finding | Why it matters |
|---|---|
| **`antra run` auto-assigns a port, then blames the user** | For a server that hardcodes `listen(3000)` and ignores `process.env.PORT` — ordinary Node — Antra registers 4000, prints the URL as though ready, and the 503 says *"is your server running?"* when it is running, on 3000. The port warning scrolls past, everything after it reads as success, and the one-flag fix is never repeated where it is actually needed. Details and repro in `tests/user-test-2026-10-01.md`. |
| **`antra trust` cannot install on a Mac without a GUI** | `antra trust --user-level` fails with *Failed to install to user keychain* and `sudo antra trust` with *Could not install CA automatically* — on a real Mac and on a runner. Found 2026-10-01. If this also happens for a user at a normal desk it is a first-run blocker, and it needs a real session to tell; the A2 test could not reach it because the trust flow was never completed by hand. Worth checking on a Mac with a real login before v0.6.4, and it is the one finding here that A1's design cannot resolve. |
| **A stale route survives a hard kill** | After `SIGKILL` rather than Ctrl+C, `antra list` keeps showing the route and `doctor` counts it as active. `antra prune` exists for exactly this; nothing points a user at it. |
| **The installer's download did not finish once** | The v0.6.3 installer stalled on *Downloading antra-aarch64-apple-darwin (v0.6.3)* for several minutes on the A2 test machine. Not verified as a bug — possibly that machine's network — and CI's Release Check installs the same script successfully. Worth one clean run from a fresh `HOME` before v0.6.4. |

To reproduce the Chrome finding by hand in the cloud container: Chromium is at
`/opt/pw-browsers`, Playwright is global (`NODE_PATH=/opt/node22/lib/node_modules
node script.js`), and `apt-get install -y libnss3-tools` provides `certutil`.
`antra trust --remove --yes` is fixed, so it undoes the trust itself; if a
future change breaks it again, by hand is
`rm /usr/local/share/ca-certificates/Antra-Local-CA-*.crt &&
update-ca-certificates --fresh`. You no longer need to do any of this to see
the gap — `.github/scripts/check-browsers.sh` reports it on every run.

**Deliberately not planned** (maintainer's call, 2026-09-30): the manual
Safari + Firefox pass on `docs/mvp.md`. The TLS half is machine-checked in CI
(`tests/e2e_securetransport.rs` asks Apple's own stack via `/usr/bin/curl`),
and real browsers are now covered by `.github/scripts/check-browsers.sh` —
which also makes the Linux NSS gap a standing assertion rather than a memory.
Safari proper is still the one engine that check cannot drive, because
safaridriver needs Remote Automation enabled; it reports that as a skip with the
reason. Do not re-add the manual pass to "Still owed" unprompted.

## Two things that will bite whoever touches the tests

Recorded in `AGENT.md` under "do not redo blindly", repeated here because they
are the most expensive lessons in this codebase:

- **A green suite proves nothing until you have checked that green is reachable
  by failure.** Five separate ways a check passed while proving nothing, one of
  which grepped for `"uvicorn"` — a string that also appears inside the
  `Failed to spawn 'uvicorn'` error it was meant to detect. Three more from the
  2026-10-01 work, all worth the warning:
  - An end-to-end test for `trust --remove --yes` passed against the broken
    code, because a hermetic home has no `ca.pem` and both code paths exit 0
    before reaching the prompt. It was deleted rather than kept as reassurance.
  - `tests/installer_output.sh` printed seven green ticks and reported
    **0 pass**, because `install.sh` — which the test sources — defines its own
    `ok()`, silently replacing the test's counter. Its helpers are `t_`-prefixed
    for that reason. Read the summary line, not the tick marks.
  - The **browser check went green having launched no browser at all** (ROADMAP
    C21): `npm install -g playwright` leaves the driver off node's module path,
    `require.resolve` fails, and a `skip` for a missing driver turns three
    browser lines into expected failures. `0 pass / 0 fail` looks like success.
    A check that cannot run must **fail**, and a diagnostic that hides its own
    stderr will report an absence where the truth is a permission error.
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

- `tests/installer_output.sh` — the installer suite, seconds, and it needs
  neither `cargo build` nor free ports, so it is the quickest gate in the repo:

  ```bash
  bash tests/installer_output.sh
  ```

  It is **not** in the E2E CI job yet. Wire it in when the browser workflow
  settles; it is the only check that would catch the installer's colour
  escaping again, and that bug survived four releases precisely because nothing
  ran it.

- The browser check, if you touch trust, TLS or the installer:

  ```bash
  cargo build
  ANTRA_BROWSER_HOME=/tmp/ab-local ANTRA_PORT=18997 ANTRA_HTTP_PORT=18996 \
    bash .github/scripts/check-browsers.sh
  ```

  It stands up its own daemon, upstream and route. Read three things in the
  output, not the exit code: whether the CA is **trusted in this environment**
  (if not, every browser line is expected-fail and says why), whether
  Playwright was found, and the final count. A green run with no Playwright does
  **not** mean the browsers were checked. Only `curl --cacert` is strict.

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
