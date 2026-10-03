# NEXT-SESSION.md — Instructions for the next session

> Written to be read cold, by any tool. It does not assume you were present for
> the previous conversation.
>
>
> **Direction (maintainer, 2026-09-30): make Antra "good to market", then
> release once, then GTM.** The release happened: **v0.6.4 is published**
> (2026-10-02) and carries the launch-readiness work — C17, C18, C22, C23.
> The maintainer's calls, not to be re-litigated:
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
> **Start here.** All three A2 follow-ups are fixed, **unreleased**, on the
> branch `claude/dazzling-mayer-mo3zyz` (2026-10-03) — merge it if it is not
> already on `main`: **C24** (a server with a hardcoded port now gets its
> route moved to where it really listens, and the 503 names that port instead
> of asking whether the server runs), **C25** (the daemon reaps routes whose
> process was hard-killed) and **C26** (`tests/installer_output.sh` has its own
> CI job; the installer ran clean from a fresh `HOME` on Linux). Both product
> fixes hit the **first run of every new user** with an ordinary Node server,
> so they are worth shipping before GTM traffic arrives: the next step is a
> **v0.6.5** release (Releasing, below — the tag needs a person), then GTM.

Repo: `main` is the default branch; work in a feature branch.

**Status as of 2026-10-03:** v0.6.4 is published and checked — Release Check
green on all five legs. C24, C25 and C26 are done on the branch above with
the gates green (fmt, clippy on Linux **and Windows** via a mingw
cross-check, 500 Rust tests passing, the four shell suites, the installer
suite) and every new test mutation-checked. Chrome on macOS is verified
warning-free by hand; Firefox is not installed on that machine, and the
Linux NSS gap (C19) stands by decision and is asserted on every CI run.
Nothing is owed before the v0.6.5 release except merging and tagging.

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
    ./target/debug/antra --version                 # 0.6.4

Full test commands, including the two traps that cost real time, are under
**Gates** at the bottom of this file.

## Where things stand

`v0.6.4` is published (2026-10-02), the landing site serves it, and both
install paths were checked on GitHub's macOS and Linux runners (**Release
Check**): Homebrew and `curl | bash`, latest and pinned, all report
`antra 0.6.4`. v0.6.4 carried the launch-readiness fixes (C17, C18, C22,
C23); v0.6.2 shipped
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
Rust suite is at 500 passing (2026-10-03, with C24/C25), and since C15 it
passes in full in the IPv6-less claude.ai cloud container too.
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
| **C23 — a CA rotation broke every domain already opened** | **FIXED and now covered** | Fix in #52, end-to-end test in #55. Two causes: a running daemon never reloaded the CA, and leaf certs on disk were never checked against it. Both mutation-verified, plus two tests that rotate the CA through the real HTTPS server — one per cause, each confirmed to fail against its own cause and pass against the other, and the first also asserts the retired CA no longer verifies so a cache that never noticed anything cannot pass it. The blind spot that hid this is now covered: something in CI rotates the CA underneath a live daemon. |
| **Chrome and Firefox on macOS** | **Chrome: verified clean, 2026-10-02.** Firefox: not installed here — measured gap | System Google Chrome (Playwright `channel:"chrome"`, visible) loaded `https://chrome-probe.localhost:18999/` with **no certificate warning** (`OK 200`). Preconditions asserted first: `antra trust --status` → trusted via login keychain (fingerprint `A9:48…` matches on-disk `ca.pem`), and `curl --cacert` 200 before the browser was asked. No trust-store writes — the user's already-trusted CA was used. A CA-key mismatch seen mid-verification was NOT a product bug: a stale daemon started under a hermetic throwaway `HOME` owned the port with its own throwaway CA. Firefox is not installed on this machine, so the macOS Firefox side is a measured gap, not a pass; the Linux NSS gap (C19) stands and is asserted on every CI run by `check-browsers.sh`. |
| **Then: one release** | **DONE — v0.6.4 published 2026-10-02** | All Releasing steps executed: #57 merged (11/11 checks green), tag `v0.6.4` pushed, release published (draft publish confirmed taken), Release Notes applied, formula hashes read from the release's own `.sha256` assets, formula updated in this repo **and the tap** (`ifelse-codes/homebrew-antra`), pinned-version examples bumped, Deploy Landing re-deployed and live site serves the v0.6.4 pin. **Release Check: all five legs green** — Homebrew macOS/Linux, curl installer macOS/Linux, live site. The notes state the Linux Chromium/Firefox gap with the `certutil` workaround and call out C23 as the upgrade-path fix. |

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
without a GUI authorisation dialog. **Settled by hand on 2026-10-01 (C20
closed): on a real Mac with a real login it works** — `--status` reports
`CA is trusted via your login keychain` and `--user-level` is idempotent. The
failure is specific to a headless runner, so nothing here affects users. That
is also why this leg can only assert `curl --cacert`.

The same hand-run surfaced **C22**, which no automated check would have caught:
`antra trust --user-level` printed *already trusted via your login keychain* and
then, two lines later, *✓ Current Antra CA is absent from all applicable trust
stores*. Both true, different certificates — the rotation cleanup shares a
helper with `trust --remove` whose message was hardcoded. Fixed; the helper now
names the certificate it removed.

**If you want a real macOS browser assertion**, it needs a trust store the
runner can write. Cheapest option: `security add-trusted-cert` into a temporary
keychain and point `HOME` at it. Not done here, and not needed for v0.6.4.

### C23 — the upgrade path was broken, and no check would have caught it

Found 2026-10-01 by reading one line of a manual run: `over TLS through antra:
FAILED`. `check.localhost` was a domain that had been used *before* on that
machine. A CA rotation had happened earlier, and the leaf certificate cached on
disk was still signed by the **retired** CA.

**Why it is the worst-shaped bug in this project.** A user upgrades to a
CA-rotating release, runs `antra trust`, it **succeeds** — and then every domain
they had already opened warns in the browser. Re-running `antra trust` cannot
fix it, because the rotation is the thing that already succeeded. The only cure
was deleting `~/.config/antra/certs` and restarting the daemon. Domains first
seen *after* the rotation worked fine, and that asymmetry is what makes it read
as "`antra trust` is broken".

Fixed in #52, two independent causes, both verified by mutation. Details and
evidence in ROADMAP C23. The uncomfortable part: `antra doctor` already
detected this and said *"Daemon is serving a retired CA — restart it"*. The
diagnosis shipped; the fix never did.

**The gap this exposed in the tests — now closed.** Every suite in this repo
built a fresh CA and never rotated one, so the entire class of "upgrading breaks
it" was untested, including the browser check added in #49, which would not have
caught it. `tests/ca_rotation.rs` (in #55) covers it: two tests, one per cause,
through the real `proxy::https::start_server` with a real on-disk `CertStore`.
The rotation is done by moving the version marker, which is the same lever a
newer `CA_VERSION` pulls, so the test exercises the branch a release would.

Writing that test surfaced a second lesson worth keeping: its first version
built `CertStore` as a struct literal, which skips `ensure_leaf_version` — the
whole of the disk-side fix — and failed with `BadSignature` for reasons
unrelated to the code under test. `CertStore::at()` now exists so a test can use
a temp dir *and* the real start-up path. **A test that cannot reach the real
start-up path will happily test a store that cannot occur** — the same family as
C21, where a browser check shipped green without launching a browser.

**How it was found, because the method matters more than the fix.** Not by a
suite, and not by the browser check: by a person reading one line of a manual
run — `over TLS through antra: FAILED` — for a domain that had been opened
before. Six of the eight findings this session came from someone reading actual
output rather than from a green build.

### From A2 — all three closed (2026-10-03, unreleased)

| Finding | Outcome |
|---|---|
| **`antra run` auto-assigns a port, then blames the user** | **C24.** Reproduced on v0.6.4 with a silent `listen(3000)` server: 503 for every one of 12 polls, and the 503's own fix said `--port 4000`. Now the route moves to 3000 by itself after 5 s (and back, if the assigned port answers later) and the terminal prints the `--port 3000` to use next time; a 503 that still happens names the port the server really holds. |
| **A stale route survives a hard kill** | **C25.** Reproduced on v0.6.4: listed and counted active 9 s after `kill -9`. The daemon now removes it within 5 s; `list` and `doctor` say *process exited* and point at `antra prune` meanwhile, or forever against an older daemon. |
| **The installer's download did not finish once** | **C26, Linux only.** The live installer from a fresh `HOME` finished in 1.9 s with a hash equal to the release's own. The A2 stall was on macOS and is still unexplained; the cloud container cannot reach the landing domain, so the run used the byte-identical `install.sh` from `main`. |

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

  CI runs it as its own job, **Installer Output** (C26), under `/bin/bash` on
  macOS and Ubuntu — on macOS that is bash 3.2, what a `curl | bash` user
  gets. It is the only check that would catch the installer's colour escaping
  again, a bug that survived four releases because nothing ran it.

- Clippy for Windows, from Linux, when you touch `cfg` code: the CI clippy
  job runs on Windows too, and dead-code lints differ per platform.

  ```bash
  apt-get install -y gcc-mingw-w64-x86-64-posix   # ring needs a C compiler
  rustup target add x86_64-pc-windows-gnu
  cargo clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
  ```

  macOS still cannot be cross-checked this way (`ring`'s build script); swap
  a `cfg` to compile the macOS branch on Linux instead, as `AGENT.md` says.

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
