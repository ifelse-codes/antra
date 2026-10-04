# AGENT.md — Antra Development Driver

> **READ THIS FILE FIRST.** This is the single source of truth for building Antra.
> Everything you need — architecture, phases, rules, status, commands — is here.

---

## Session kickoff

When the user says **"start next session"** (or anything equivalent), do not
ask what work is planned — read [`NEXT-SESSION.md`](NEXT-SESSION.md) and
execute its plan end-to-end. That file is the source of truth for the current
scheduled work. After it is complete, update its "current plan is done" state
in `AGENT.md` so the next kickoff points at new work.

---

## What is Antra

Antra is a native Rust developer CLI that maps stable domain names to local development servers.

```bash
antra run --domain myapp.localhost -- pnpm dev
```

Result:
```
ANTRA

✓ Proxy ready (port 443)
✓ HTTPS ready
✓ Route registered

  https://myapp.localhost
  → 127.0.0.1:5173
```

The user opens `https://myapp.localhost` and their app loads. No ports to remember.

---

## Unreleased — C27: ask once, then serve on port 443 (2026-10-04)

**Built on `claude/peaceful-curie-x1xkz8`; the macOS run is the
maintainer's** (checklist in `NEXT-SESSION.md`). Found while preparing GTM:
without root the daemon falls back to 8443, so a stranger's first run printed
`https://myapp.localhost:8443` under a README promising no port. The first
daemon start in a terminal now asks `Use port 443? [Y/n]`, runs `sudo` once,
and the daemon drops to the user right after binding 443/80. The design and
its guardrails are in `docs/security.md` (*Port 443 and `sudo`*); the
evidence is in ROADMAP C27.

**Do not redo blindly:**
- **`sudo` on Linux resets `HOME` and drops `XDG_RUNTIME_DIR`.** Measured
  with `sudo -u <user> env` in the container: `HOME` becomes the *target's*
  home and the runtime dir is gone. That made the hint Antra printed for years
  — `sudo antra proxy start` — start a daemon whose socket sat in `/root`,
  invisible to the user. macOS `sudo` keeps `HOME`, which is why it was never
  seen there. `platform::sudo::adopt_invoking_user_paths` fixes the paths for
  every `antra proxy` command under `sudo`; the first-run offer also passes
  the CLI's own values through `sudo -- /usr/bin/env …`.
- **macOS lets non-root bind ports below 1024 only on `0.0.0.0`.** The daemon
  binds `127.0.0.1`/`::1` on purpose (nothing on the LAN may reach it), so it
  needs root for 443 on macOS as on Linux. Do not "fix" C27 by binding
  `0.0.0.0`.
- **Drop privileges before the tokio runtime exists.** `setuid` and
  `set_var` are only safe single-threaded; `proxy::execute` does both before
  building the runtime and passes the bound listeners in through
  `DaemonConfig::prebound`. A port missing there is bound again as the user
  and falls back as usual.
- **Every path the launcher and the daemon share must use the *acting* uid.**
  The `proxy start` launcher stays root; the daemon it starts does not. The
  `/tmp/antra-<uid>` socket fallback used `geteuid()`, so for a long `HOME`
  the two would have looked in different directories — reproduced: the
  launcher said *failed to start within timeout* while the daemon ran.
- **Only the `proxy start` launcher's daemon drops root.** It marks the
  child with `ANTRA_DROP_TO_USER`. Without the marker, `sudo antra run` (a
  root CLI that auto-starts the daemon) got a daemon in the user's paths
  while the CLI kept looking in root's — caught in review, and checked: with
  the marker, that daemon stays root, in root's paths, and serves 200.
- **Anything the root launcher creates in the user's home must be the
  user's.** `open_for_append` made the log directory as root; on macOS that
  directory also holds the socket, so from a fresh home the dropped daemon
  died with *Permission denied*. Test the elevated path from an **empty**
  `HOME` with no runtime dir — a home that already has `antra` in it hides
  this.
- **How to test the elevated path without giving anyone `sudo`.** The
  session's permission check refuses to add a sudoers entry for a test user.
  What works instead: a recording fake `sudo` first on `PATH` (captures the
  argv the CLI builds), a small Python `pty` driver to answer the prompt, then
  that argv replayed as root under `env -i HOME=/root SUDO_UID=… SUDO_GID=…
  SUDO_USER=…` — the environment `sudo` leaves on Linux — and the user's CLI
  run with `su <user> -c`. A real password prompt is still unverified.

---

## Release — v0.6.5 (2026-10-04): the A2 follow-ups (C24–C26)

**The three findings from the 2026-10-01 stranger test are closed and
shipped in v0.6.5.** Each was reproduced against the released v0.6.4 binary
first, in the cloud container, and then against the fix.

**Release mechanics:** #58 (fixes), #59 (bump), tag `v0.6.5` on `e7b6ce1`
pushed by the maintainer (the agent's push got HTTP 403, as documented), draft
published by the maintainer, Release Notes run by the agent, assets checked
three ways with sizes, #60 (formula + pins) and ifelse-codes/homebrew-antra#3
(tap). Deploy Landing and Release Check (five legs) green. Nothing in the
release needed a local machine; two steps needed a person: the tag and the
publish click.

- **C24 — a hardcoded-port server.** `antra run` auto-assigned 4000 to a
  `listen(3000)` server and its 503 asked "is your server running?". After an
  auto-assign, `port_watcher::confirm_auto_port` asks the OS which ports the
  child's process group holds and moves the route (503 → 200 in ~5 s, no
  flag), moving it back if the assigned port answers later; the 503 page
  asks the route's owner instead of the user.
- **C25 — a route outliving a hard kill.** The daemon reaps managed routes
  whose owner PID is dead every 5 s; `list`/`doctor` flag them meanwhile.
- **C26 — `tests/installer_output.sh` runs in CI**, as its own job under
  `/bin/bash` (bash 3.2 on macOS). The live installer ran clean from a fresh
  `HOME` on Linux in 1.9 s.

**Do not redo blindly:**
- **A zombie answers signal 0.** `kill(pid, 0)` succeeds until the parent
  reaps it. The child of a SIGKILLed `antra run` is reparented, and the cloud
  container's init (`process_api`) reaps lazily — seconds — so a dead server
  read as *running*. `platform::is_pid_alive` reads `/proc/<pid>/stat` on
  Linux for that reason. It also treats **EPERM as alive**: it means the
  process exists and is someone else's.
- **`lsof` ORs its selectors unless given `-a`.** `lsof -g <pgid> -iTCP
  -sTCP:LISTEN` lists every TCP listener on the machine; `-a` makes it the
  group's. Verified in the container against a `setsid` server.
- **Do not shorten `AUTO_PORT_GRACE` below 5 s.** A dev command that starts
  an API before the app shows the API's port first; with a 2 s grace the
  route was pinned to the API. The move-back covers a slow app; the grace
  keeps the common case from flapping at all.
- **Process-group lookups need the child to lead its group.** `antra run`
  spawns with `process_group(0)`, so `pgid == child pid` and `npm → node`
  descendants are covered. Anything that spawns differently breaks
  `group_listening_ports`.
- **The landing domain is unreachable from the cloud container** (the proxy
  answers 403), GitHub is not. To run the live installer there, fetch
  `install.sh` from `raw.githubusercontent.com/.../main/` — it is
  byte-identical to `landing/install.sh`, which `tests/installer_output.sh`
  asserts.
- **Clippy for Windows runs from Linux** with `gcc-mingw-w64-x86-64-posix` and
  the `x86_64-pc-windows-gnu` target (see Gates in `NEXT-SESSION.md`). Use it
  after touching `cfg` code: dead-code lints differ per platform and CI runs
  clippy on Windows.

---

## Release — v0.6.3 (2026-09-30)

**One macOS fix, no behaviour change on Linux or Windows.**

**Release mechanics, all verified, all without a local machine:**
- Tag `v0.6.3` on `643eecf` (the merge of #44), pushed by the maintainer. The
  release workflow built five targets and the draft in ~5 min.
- **The first publish did not take**: the release stayed a draft and
  `releases/latest` still pointed at v0.6.2. Check `draft` through the API
  (`list_releases` shows drafts; the public `releases/tags/<tag>` endpoint
  404s on one) rather than trusting that the button was pressed. The second
  publish worked, and **Release Notes** set the description from
  `docs/releases/v0.6.3.md` and read it back equal.
- Every asset checked three ways (`.sha256` asset, fresh download, GitHub's
  digest) with sizes; `file` reports the right format for each. The formula
  went to both copies — this repo (#45) and the tap
  (ifelse-codes/homebrew-antra#2) — and each URL it builds resolves to a
  matching file.
- **Deploy Landing** ran by itself on the merge of #45 and its live-domain
  check passed. **Release Check** `0.6.3`: Homebrew on macOS and Linux, and
  `curl | bash` latest and pinned on both, all report `antra 0.6.3`; the live
  site passes with the v0.6.3 pin example.


**`antra service install` keeps one supervised daemon on macOS (ROADMAP
C16).** Confirmed on a GitHub `macos-latest` runner before fixing: launchd ran
the job 7 times in 60 s and supervised no pid, because the plist ran `antra
proxy start` without `ANTRA_DAEMON=1` — C14's bug in launchd form. The plist
now sets it; install also unloads a loaded job, refuses to load beside a
daemon already running outside the service, and waits for the socket before
saying *started*. **Service (macOS)** checks it on a real Mac on every change
to `service.rs`: 17/0, including an upgrade over a v0.6.2 plist.

**Do not redo blindly:**
- **A GitHub `macos-latest` runner is a real Mac for launchd.** Its shell is in
  the `Aqua` session (`launchctl managername`), so `launchctl load` of a
  LaunchAgent works and `launchctl print gui/$(id -u)/<label>` reports `runs`,
  `state` and `pid`. `runs` counts starts since the job was loaded; `pid`
  missing while `state` is `not running` between relaunches is the C16
  signature.
- **launchd stops only the job's own process group on unload.** A daemon
  forked into its own group (what `proxy start` does without
  `ANTRA_DAEMON=1`) outlives `launchctl unload` — which is why upgrading over
  a v0.6.2 job must hand over rather than load beside it.
- **The macOS install code does not compile on Linux**, and cross-checking
  for `x86_64-apple-darwin` fails in `ring`'s build script. To type-check it
  here, replace its `#[cfg(target_os = "macos")]` with `#[allow(dead_code)]`,
  run clippy, and restore the file; it depends on nothing macOS-only.

---

## Release — v0.6.2 (2026-09-30)

**Two Linux fixes, no behaviour change.** C14 makes `antra service install`
work on Linux; C15 lets the daemon start on a host without IPv6. C13 is a
test-only change.

**Release mechanics, all verified after the fact — and all run without a
local machine:**
- Tag `v0.6.2` on `f8aa404` (the merge of #39), pushed by the maintainer: a
  cloud agent session's git proxy refuses tag pushes with HTTP 403. The
  release workflow built all five targets and the draft in ~4 min.
- Published 2026-09-30. The description was set by the new **Release Notes**
  workflow from `docs/releases/v0.6.2.md` and read back equal.
- Every asset was checked three ways — the `.sha256` asset, a fresh download,
  and GitHub's own asset digest — with sizes equal to the asset sizes. `file`
  reports the right format for each. The published `antra-x86_64-linux` ran in
  the IPv6-less cloud container (C15), served a real upstream over HTTPS, and
  passed all 28 service checks (C14).
- **The Homebrew tap was four releases stale.** `brew install
  ifelse-codes/antra/antra` reads `ifelse-codes/homebrew-antra`, which was
  still on 0.2.3; every release since had updated only this repo's
  `Formula/antra.rb`. Both now carry the same v0.6.2 formula
  (ifelse-codes/antra#40, ifelse-codes/homebrew-antra#1).
- Landing deployed by the new **Deploy Landing** workflow as a production
  deployment (`8090305e`). Its own check then confirmed the live domain serves
  the new `install.sh` (on the second attempt, ~10 s after the deploy — the
  retry is load-bearing), as `text/plain`, with the 404 page and all six
  security headers.
- **Release Check** on GitHub runners: Homebrew on macOS (Apple Silicon,
  `/opt/homebrew/Cellar/antra/0.6.2`) and on Linux, and `curl | bash` latest
  and pinned on both, all report `antra 0.6.2`; `brew test antra` passes; the
  live-site check passes with the v0.6.2 pin example.


**`antra service install` works on Linux (ROADMAP C14).** It was broken in two
layers, and fixing only the one the handoff named would have shipped a service
that restarts forever:

1. The unit went to `~/.config/antra/systemd/user/`, which `systemctl --user`
   does not search, and `enable` ran with no `daemon-reload`. It now goes to
   `$XDG_CONFIG_HOME/systemd/user/` and install runs reload → enable → start.
2. `antra proxy start` **forks the daemon and exits**. Under `Type=simple`
   systemd reads that exit as the service stopping and SIGTERMs the rest of the
   cgroup — the daemon — then `Restart=always` starts the cycle again. The unit
   now sets `Environment=ANTRA_DAEMON=1`, so `proxy start` runs the daemon in
   the foreground, which is exactly what the CLI's own auto-start does.

Install also stopped lying: it prints *started* only once the IPC socket
answers, rolls the unit file back to what it found when reload or enable fails,
refuses to start a second daemon beside one already running (and says how to
hand over), and names the missing systemd user session instead of printing a
bare D-Bus error. Units left at the old path were never loaded; install and
uninstall delete them and status mentions them.

**Verified against a real `systemd --user` manager**, with
`tests/manual_service_linux.sh`: 28 checks, 28/0 on the fix and 10/18 on v0.6.1
on the same box. Five new unit tests in `cli/service.rs`; each was mutated to
confirm it goes red.

**The daemon starts on a host without IPv6 (ROADMAP C15).** Every listener
bound `::1` beside `127.0.0.1` and treated *any* failure as "port in use", so
on a kernel with no IPv6 (`EAFNOSUPPORT`) or no `::1` (`EADDRNOTAVAIL`) the
daemon refused to start with `Both 443 and 8443 are in use` on free ports, and
`is_port_available` called every port taken. `util::port::loopback_binds` now
makes the one decision for the sync check and the three async listeners
(`proxy::https::bind_loopback`): `::1` is dropped only for those two errors,
and a taken or forbidden `::1` is still an error, so the reason for binding it
— nothing else may answer `localhost` on IPv6 — is kept wherever IPv6 exists.
Verified in the IPv6-less cloud container with the real binary: the daemon
serves HTTPS and the redirect, the whole Rust suite goes from 7 failures to
0, and the 28 service checks pass. Five hermetic tests pin the decision so CI
machines, which have IPv6, still guard it; five mutations were each caught.

**Do not redo blindly:**
- **Any service manager that runs `antra proxy start` needs `ANTRA_DAEMON=1`.**
  Without it the managed process is a launcher that exits in ~100 ms. The
  launchd plist had the same shape (ROADMAP C16, fixed in v0.6.3).
- **Running `systemd --user` in a container that was not booted with systemd.**
  `systemd --user` exits 1 silently; `strace` shows it checking
  `/run/systemd/system/`. This is enough to get a real user manager:
  ```bash
  mkdir -p /run/systemd/system
  export XDG_RUNTIME_DIR=/run/user/$(id -u); mkdir -p -m 700 "$XDG_RUNTIME_DIR"
  setsid systemd --user >/tmp/sd-user.log 2>&1 &
  systemctl --user is-system-running   # → running
  ```
  `systemd-analyze --user unit-paths` prints the search path without a manager.
- **The claude.ai cloud container has no IPv6** (`socket(AF_INET6)` →
  `EAFNOSUPPORT`). Since C15 that no longer matters: the full Rust suite and
  the shell suites pass there with the real binary. It is also the one place
  that exercises the IPv4-only path for real, so run the suites there after
  touching `loopback_binds`. **Never "fix" a bind failure by dropping `::1`
  outright** — on a dual-stack Mac that reopens the hole `::1` closes.
- **The cloud container can restart between turns** and takes the user
  manager with it: `systemctl --user` then says *Connection refused*. Re-run
  the recipe above; `tests/manual_service_linux.sh` refuses to run without a
  manager rather than printing 14 failures that read like product bugs.
- **The service and the CLI find each other through `$XDG_RUNTIME_DIR`.**
  `socket_path()` prefers it, and the user manager always sets it. A shell
  without it (`su` without `-l`, some cron or container shells) looks under
  `data_local_dir()` instead, cannot see the service's daemon, and auto-starts
  a second one. Check `echo $XDG_RUNTIME_DIR` before debugging anything else.
- **A service restarts on idle.** The daemon exits after 10 minutes with no
  routes and `Restart=always` brings it back 5 s later — observed: *Idle
  timeout reached* at 09:52:34, *Daemon ready* again at 09:52:39,
  `NRestarts=1`. Harmless, but it reads like a crash in `NRestarts`.

---

## Release — v0.6.1 (2026-09-29)

**Two user-facing fixes and one behaviour change.** Both fixes are bugs a user
could hit without doing anything unusual.

- **The daemon could refuse to start on a long `$HOME`.** `socket_path()`
  built its Unix socket path from `dirs::data_local_dir()`, which on macOS is
  `$HOME/Library/Application Support` — 50 bytes of fixed overhead against a
  104-byte `sun_path`. Any home directory longer than ~54 characters (a long
  username, a CI runner's `mktemp -d`) produced a path that could not be bound,
  and the failure was the bare `Error: path must be shorter than SUN_LEN`,
  naming neither the path nor the limit. It now falls back to
  `/tmp/antra-<uid>/<fnv1a-of-home>/d.sock`. ROADMAP C7.
- **The IPC socket was briefly reachable by any other local user.** It was
  `bind`-ed and only *then* tightened to `0o600`, so between the two it sat at
  the process umask. `platform::ensure_private_dir` now creates the parent
  chain `0o700` before the bind, and chowns each component to the invoking user
  so `sudo antra proxy start` still leaves the CLI able to reach its own
  daemon. ROADMAP C10.
- **`antra service status` told Linux users to start a service that was never
  installed.** It inferred installed-versus-not from a single
  `systemctl --user is-active` code, which reports `inactive` both for a unit
  that is stopped and for one that does not exist. The result was
  `Run: systemctl --user start antra-proxy` on a machine with no service. The
  unit file's presence now decides "installed".

**Behaviour change:**

- **`antra dev` honours a port pinned in your dev script.** A project whose
  `package.json` says `{"dev": "vite --port 3001"}` was registered on Vite's
  default 5173, so the URL pointed at a port nothing listened on. Precedence is
  now: explicit `--port` > a port pinned in the dev script > the framework
  default > auto-assign. ROADMAP C9.

**Known issue, not fixed here:** `antra service install` writes the systemd
unit to `~/.config/antra/systemd/user/`, which is not a path `systemctl --user`
searches, and then runs `systemctl --user enable` with no `daemon-reload` or
`--user link`. Install is likely broken on Linux for that reason. ROADMAP C14. *Fixed in v0.6.2 — see above.*

**Release mechanics, all verified after the fact:**
- Tag `v0.6.1` → the release workflow built all five targets; the draft was
  published with real release notes. <https://github.com/ifelse-codes/antra/releases/tag/v0.6.1>
- The two binaries that could be checked were downloaded from the **public**
  release URL and re-hashed: `antra-aarch64-apple-darwin` (`48d18897…`) runs
  and reports `antra 0.6.1`; `antra-x86_64-linux` (`d943d92d…`) is a real ELF
  x86-64 executable. Formula sha256s come from the release's own `.sha256`
  assets rather than a local hash, so a typo cannot creep in.
- One download arrived 8.3 MB against an expected 10.3 MB — a truncated
  transfer — and appeared to mismatch its checksum. The checksum was right and
  the download was not. **Check size alongside hash.**
- `releases/latest` now redirects to `v0.6.1`, so a fresh
  `curl -fsSL https://antra.iifelse.com/install.sh | bash` installs v0.6.1 with
  no code change; only the copy-pasteable examples were updated.
- Landing redeployed as a **production** deployment (`b6482eed`), from
  `landing/` and with an explicit `--branch main`. Verified against the live
  domain rather than the deployment URL: `/install.sh` and the index both read
  v0.6.1, `/install.sh` still returns `Content-Type: text/plain`, unknown paths
  return 404, and CSP/HSTS/`X-Frame-Options`/`nosniff`/`Referrer-Policy`/
  `Permissions-Policy` are all still present.

---

## Session Handoff — 2026-09-27 (afternoon)

**`v0.6.0` is published** (tag `v0.6.0`, PRs #14–#16, formula updated from the release's own sha256s, landing redeployed as a *production* deployment). ROADMAP #32 (`antra logs`) and #33 (shared upstream client) are done, along with the two resolver fixes (wrong-subcommand sudo hint, underscore domains) and the landing security headers + real 404. The working tree is clean. The v0.5.0 CA work it builds on is documented in `docs/security.md` (CA versioning and rotation).

**What changed in v0.6.0:**
- `antra logs [-f] [--lines N]`. The daemon's output used to go to `/dev/null` on the auto-start path, so "HTTPS server failed" existed only in the code that printed it; several user-test sessions had asked for this command. One log path now serves every writer — `util::logs` — including the launchd plist, which pointed at `~/.config/antra/daemon.log` while the CLI wrote `data_local_dir()/antra/daemon.log`: two different files on macOS. Log is truncated past 5 MiB rather than rotated. `doctor` tails the last errors.
- One pooled upstream client in `ProxyState` instead of one per request. `tests/upstream_pool.rs` counts TCP connections through the real TLS server: 4 sequential requests → 1 upstream connection, and the old per-request build → 4 (verified by temporarily reverting).
- A denied `/etc/hosts` write no longer suggests `sudo antra alias <domain> <port>` regardless of what you ran; the suggestion follows the domain suffix.
- Underscores are rejected in domain names. They were accepted, which is the same class of bug as the CA SAN: a `_` cannot appear in a `dNSName`, so the failure moved from a clear CLI error to a certificate no strict verifier accepts.
- Landing site: CSP, HSTS, `X-Frame-Options`, `nosniff`, `Referrer-Policy`, `Permissions-Policy`, and a real 404 (unknown paths returned the homepage with a 200).

**Verified this session:**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, full suite green in a disposable HOME (233 executions, 0 failures).
- `antra logs` against a live daemon prints its real output; the empty case explains itself and exits 0.
- The landing site was re-checked in a real browser after the CSP landed: Inter and JetBrains Mono still load, the Plausible tag is present, zero page errors, `/nonexistent` → 404.

**Do not redo blindly:**
- **Green in this repo has meant several different things, and twice it meant nothing.** Count the assertions that *can* fail before trusting a result. The `select_resolver` test asserted a count of 1 that could only ever be 2. A `grep -q "$opt"` with `opt="--domain"` made grep parse the pattern as its own flag, so 7 option assertions failed regardless of the help text. A `-q` piped into `grep -q` fed nothing to the second grep, and its `else` branch also called `log_pass`. `test_e2e_real_server` started its server with `timeout 5` — GNU coreutils, absent on macOS — so the command died instantly and the `else` branch reported a pass; the one true end-to-end test in that suite had never run a server. `uvicorn command used` grepped for a string that also appears inside the `Failed to spawn 'uvicorn'` error, so it passed on the failure. Verify a new assertion can fail: serve the wrong body and confirm exactly one check goes red.
- **A toolchain gate is not a timeout, and detection assertions need no gate at all.** `need "python"` skips when the binary is missing; it does nothing when the binary exists and does not exit. `antra dev` stays in the foreground running the project's dev command, so a server command blocks forever — 45 minutes on a CI runner, invisible on this Mac only because `python` is absent here and the test skipped. All 37 such calls in `e2e_all_features.sh` are now capped by `run_antra_capped` (38 now, counting one added later for the Flask port split); that helper's kill is deliberately surgical (see below) and a new suite must not regress it.
- **What decides whether a test needs a `need` gate: does it assert the spawn line or a route, or only detection?** The spawn line is printed only after a successful spawn, so an assertion on the chosen command or on a registered route proves nothing on a machine without that toolchain — it fails, or worse, it passes on the `Failed to spawn '<x>'` error that contains the same string. A test asserting only on *detection* needs no gate at all, which is why the Node framework tests are fine ungated. Ten tests are gated and twenty-five are not; the ungated ones asserting a command are enumerated in ROADMAP C12.
- **When capping a process, mind the two children.** `antra` starts the long-lived daemon *and* the project's dev command. Only the dev command leads its own process group (`pgid == pid`); the daemon shares antra's. Killing on that predicate takes the server down and leaves the daemon running — verified against a real `python -m http.server`. Killing the group blindly would take the daemon too and force every later test to rebind its ports.
- **Ports on this machine are not free.** 8080 and 8443 are held by someone else's `ssh`; 3001 is the Agent Orchestrator daemon; 5000 is macOS Control Center (`ControlCe`). Do not kill them. Assert the port a detector *chose* rather than a successful bind, and use the suites' `free_port` probe for anything that has to actually run.
- **Both test harnesses key `/tmp` off the worktree name now, and that is load-bearing.** They were fixed paths. Git worktrees isolate tracked files; they do not isolate `/tmp`. The Rust harness's failure was the nastier one: two concurrent `cargo test` processes interleaving between the CA certificate and key writes left a `ca.pem` whose public key did not match `ca-key.pem`, and every leaf signed afterwards failed verification — surfacing as `LibreSSL ... asn1 encoding routines:CRYPTO_internal:EVP lib`, which reads as a product certificate bug and is not one. `rm -rf /tmp/antra-e2e` is what identifies it. Before running `cargo test` in parallel, check no other checkout is doing the same.
- **`pkill -f "<pattern>"` can kill your own shell.** The pattern matches the full command line of the process running `pkill`, which includes the pattern itself. Use the bracket form — `pkill -f "antra[ ]proxy start"` — or match on a PID.
- **A failing test can be the test being right.** `e2e_next_sprint.sh` expects `antra service status` to say "not installed" on a machine where the service was never installed, and it fails on Linux because the product says "installed but not running" instead. The cause is `cli/service.rs` inferring installed-vs-not from a single `systemctl --user is-active` code, which does not separate an absent unit file from a stopped one. The fix is in the product — a test that catches a real wrong-advice bug should not be edited to match it.
- Do not kill unrelated processes to make a test pass.
- If a test daemon lingers after a run, it is an orphan from a temp HOME — check its open files (`lsof -p <pid> | grep antra`) before stopping it, so you do not kill the user's real daemon.
- Overriding `HOME` for a test run breaks `cargo` unless you also pass `CARGO_HOME` and `RUSTUP_HOME`, or rustup fails with "could not choose a version of cargo to run". Copy the local gate below verbatim.
- The CA rules from the previous handoff still hold: existing installs rotate once and re-prompt; never add a SAN back to the CA.

**Reproduce the local gate:**
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
# Capture the real toolchain paths first: once HOME is redirected, rustup
# cannot find them and cargo fails with "could not choose a version".
CARGO_HOME_REAL="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME_REAL="${RUSTUP_HOME:-$HOME/.rustup}"
TEST_HOME=$(mktemp -d /tmp/antra-test.XXXXXX)
HOME="$TEST_HOME" CARGO_HOME="$CARGO_HOME_REAL" RUSTUP_HOME="$RUSTUP_HOME_REAL" \
  cargo test -- --test-threads=4
rm -rf "$TEST_HOME"
```

The shell suites need a **short** `HOME`, for the `sun_path` reason above, and
ports that are free on this box:
```bash
export HOME=/tmp/antra-shell-e2e ANTRA_PORT=18443 ANTRA_HTTP_PORT=18080
mkdir -p "$HOME"
for f in tests/e2e_*.sh; do bash "$f"; done
```

**One trap this session cost time:** `wrangler pages deploy .` infers the branch from git. Deployed from a feature branch it creates a *branch* deployment and prints an alias URL — production does not move, and the site silently keeps serving the old installer. Deploy from `main`, or pass `--branch main`, and check the live domain afterwards rather than the deployment URL.

**Next actions:** the shell e2e suites are in CI (ROADMAP C8, v0.6.1) — run them locally with `ANTRA_PORT`/`ANTRA_HTTP_PORT` pointing at a free port and a short hermetic `HOME`, per **Gates** in `NEXT-SESSION.md`. The manual Safari + Firefox pass on `docs/mvp.md` is **not** planned (maintainer, 2026-09-30): real-browser coverage is now automated in `.github/scripts/check-browsers.sh`, which records the Linux Chrome/Firefox certificate warning as an expected failure rather than skipping it. `tests/installer_output.sh` is a fifth shell suite and is **not** yet in the E2E job — wire it in when the browser workflow settles.

---

## Project Status

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 0 | Scaffolding & Docs | ✅ DONE | 57 files, CLI skeleton, all docs |
| 1 | Minimal HTTP Reverse Proxy | ✅ DONE | HTTP proxy, route lookup, X-Forwarded-*, 502/503 errors |
| 2 | CLI Process Runner | ✅ DONE | `antra run` spawns child, registers route, cleans up on exit |
| 3 | WebSocket / HMR | ✅ DONE | Raw TCP tunnel, upgrade detection, loop detection |
| 4 | HTTPS / TLS | ✅ DONE | CA generation, SNI cert cache, TLS termination, HTTP→HTTPS redirect |
| 5 | Domain Resolution | ✅ DONE | .localhost no-op, .test/custom via hosts file, domain validation |
| 6 | Root CA Trust | ✅ DONE | os-truststore, install/status/remove, user prompts |
| 7 | Daemon + IPC | ✅ DONE | Unix socket IPC, JSON protocol, auto-start, idle shutdown |
| 8 | Route Management & DX | ✅ DONE | list, open, doctor, clean, alias commands |
| 9 | Configuration | ✅ DONE | antra.toml parsing, `antra dev` command, CLI flag overrides |
| 10 | Cross-Platform Hardening | ✅ DONE | Windows fixes, platform abstractions, CI/CD, release workflow |

**Current state:** Phases (0-10) are implemented and `v0.6.5` is published. The four shell e2e suites are green and all four now run in CI on macOS and Ubuntu; they went from 86 passing / 128 failing assertions to 206 / 0 / 9 skipped, and the Rust suite is at 421 passing. ROADMAP C4–C17 are done; C14 (`antra service install` on Linux) and C15 (the daemon on a host without IPv6) shipped in v0.6.2, and C13 turned out to be a test waiting for a spawn line on runners without pnpm. C16 (the launchd plist relaunching `proxy start` every 10 s) is fixed in v0.6.3, verified on a macOS runner. C17 (the busy-port advice named a fix that does not work, and Flask ignored `--port`) shipped in v0.6.4, as did C18 (installer colour codes, `trust --remove --yes`), C22 and **C23** — `v0.6.4` is the release that finally carries the launch-readiness fixes to users. The manual Safari + Firefox pass on `docs/mvp.md` is deliberately not planned (maintainer, 2026-09-30) and must not be re-added to the owed list unprompted; its machine-checkable half is covered by `tests/e2e_securetransport.rs` and, for real browsers, by `.github/scripts/check-browsers.sh`, which is green on both platforms and **does** reproduce the Linux gap on every run (Chrome `ERR_CERT_AUTHORITY_INVALID`, Firefox `SEC_ERROR_UNKNOWN_ISSUER`). It is also deployed-safe: `landing/install.sh` went out with the fix and the live site serves it. **Open by decision:** `antra trust` writes only the system trust store, so Chrome and Firefox on Linux still warn, because Phase 6 excludes NSS modification (ROADMAP C19). **Closed (C20):** `antra trust` installs under `sudo` on a Linux runner but fails on `macos-latest` in both paths — **settled by hand on 2026-10-01 (C20): it works on a real Mac with a real login**, so the failure is specific to a headless runner and affects no user. That hand-run also surfaced **C22**, a message bug no automated check would catch: `antra trust --user-level` printed *already trusted* and then *✓ Current Antra CA is absent*, because the post-rotation cleanup shares a removal helper with `trust --remove` whose success line was hardcoded to say "Current". The helper now names the certificate it removed. **C23, fixed in #52, is the one that mattered:** a CA rotation left HTTPS broken for every domain a user had already opened, *after* a successful `antra trust`, because a running daemon never reloaded the CA and leaf certificates on disk were never checked against the current one. Found by a person reading one line of a manual run; no suite in this repo would have caught it, because they all build a fresh CA and never rotate one. `tests/ca_rotation.rs` (in #55) now covers that class — it rotates the CA underneath a live daemon through the real HTTPS server, one test per cause. **`v0.6.5` (2026-10-04)** ships the three A2 follow-ups — C24 (a hardcoded-port server gets its route moved to where it really listens, and an honest 503), C25 (the daemon reaps routes whose process was hard-killed) and C26 (the installer suite in CI); the Rust suite is at 500 passing. Release Check green on all five legs. **Unreleased: C27** (ask once, then serve on 443 as the user) is built on a branch; next is its Mac run, then v0.6.6, then GTM. **Read C21 before touching the browser check:** it went green five times while launching no browser, and each cause was an assumption about the environment standing in for an observation.

### Landing Page

- **Location:** `landing/index.html`
- **Deployed to:** Cloudflare Pages → `https://antra.iifelse.com`
- **Project name:** `antra-landing`
- **Design language:** Mudra (dark, surgical, violet accent)
- **To update:** merge the change to `main`. The **Deploy Landing** workflow (`.github/workflows/landing.yml`) makes a production deployment of `landing/` only and then checks the live domain serves that commit. It needs the repository secrets `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`, and can be re-run by hand from the Actions tab. By hand, the equivalent is `wrangler pages deploy . --project-name antra-landing --branch main` **from `landing/`** — from the repo root it would publish the whole tree, and without `--branch main` from a feature branch it makes a preview deployment
- **Last deployed:** 2026-10-01, automatically by the **Deploy Landing** workflow on the merge of #49, which carried the installer colour fix. Its live-domain check passed, and `https://antra.iifelse.com/install.sh` was fetched afterwards to confirm the deployed copy is the fixed one: 0 raw-colour `echo` lines, `say` present. (The previous deploy, 2026-09-30, carried the v0.6.3 assets on #45; **Release Check** confirmed the v0.6.3 pin example.)

---

## Critical Rules — DO NOT DEVIATE

### Naming
- Product name: **Antra** (never Antara)
- Binary: `antra`
- Config dir: `~/.config/antra/`
- Config file: `antra.toml`

### Architecture
- Language: Rust only. No Node.js, Python, or other runtimes.
- Async runtime: Tokio
- HTTP: hyper 1.x
- TLS: rustls + rcgen (Rust-native, no openssl)
- WebSocket: `copy_bidirectional` (raw tunnel, not frame-level)
- Route storage: In-memory `RwLock<HashMap>` (never read disk per request)
- CLI: Clap 4 derive

### Safety
- Never silently modify `/etc/hosts`
- Never silently install system certificates
- Never kill unrelated processes
- Never expose private keys
- Never log credentials or cookies
- Always prompt before system changes

### Scope
- Read `docs/mvp.md` for in-scope / out-of-scope items
- Each phase in this file has explicit **Exclusions (DO NOT BUILD)** — follow them
- Do not add features not in the current phase

### Process
- Build compiles with zero errors and zero warnings before moving to next phase
- Test manually after each phase
- Update this file's status table when a phase completes
- Do not accumulate untested changes
- `landing/install.sh` must stay byte-identical to the root `install.sh` — it is the copy users actually download. The two drifted during the v0.5.0 release (only the version-pin comments), and nothing caught it. If you touch one, `cp install.sh landing/install.sh` and re-deploy the site with `wrangler pages deploy . --project-name antra-landing`.
- A version bump touches `Cargo.toml`, `Cargo.lock`, `README.md` (badge + status line), `install.sh` **and** `landing/install.sh`, `landing/index.html` (pin example), and `Formula/antra.rb` (needs the sha256s from the published release, so it is a separate PR *after* the release) — **plus the same file in the tap repo `ifelse-codes/homebrew-antra`**, which is what `brew install ifelse-codes/antra/antra` actually reads. It sat on 0.2.3 through v0.6.1 because only this repo's copy was updated.

---

## Key Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Default TLD | `.localhost` | Browser-native, secure context, no hosts file |
| HTTPS | First-class from Phase 4 | Required for .test, custom domains |
| CA generation | `rcgen` | Rust-native, no openssl dependency |
| TLS | `rustls` + `tokio-rustls` | Memory safe, fast |
| WebSocket | `copy_bidirectional` | Transparent, minimal overhead, HMR works |
| Route storage | `RwLock<HashMap>` | Fast, no disk I/O per request |
| IPC | Unix socket / Named pipe | Platform-native |
| Daemon | Background, auto-start | Zero-config UX |

---

## How to Run

```bash
# Build
cargo build

# Run CLI
cargo run -- --help
cargo run -- run --domain myapp.localhost -- pnpm dev
cargo run -- list
cargo run -- doctor
cargo run -- proxy start
cargo run -- trust
```

---

## Phase 1 — Minimal HTTP Reverse Proxy ✅ DONE

### Verified
- HTTP proxy forwards requests to upstream by Host header
- 502 Bad Gateway for unknown domains
- 503 Service Unavailable for dead upstreams
- X-Forwarded-For, X-Forwarded-Proto, X-Forwarded-Host headers set correctly
- Host header rewritten to upstream address
- `antra proxy start --route domain:port` works

---

## Phase 2 — CLI Process Runner ✅ DONE

### Verified
- `antra run --domain X --port Y -- <cmd>` spawns child and registers route
- Proxy forwards requests to child process (200 OK)
- Route removed when child exits
- No orphan processes
- Signal forwarding via nix (SIGTERM to process group)

---

## Phase 3 — WebSocket / HMR Support ✅ DONE

### Verified
- WebSocket upgrade detection (Upgrade: websocket + Connection: upgrade)
- Raw TCP tunnel to upstream (hyper HTTP client doesn't support upgrades)
- `hyper::upgrade::on()` captures client upgrade connection
- `serve_connection_with_upgrades()` enables server-side upgrades
- `tokio::io::copy_bidirectional()` tunnels data between client and upstream
- Loop detection via `X-Antra-Hops` header (max 5 hops)
- Forwarded headers (X-Forwarded-For, X-Forwarded-Host, X-Forwarded-Proto) on WS requests
- `cargo build` with zero warnings

### Implementation Notes
- Used raw TCP for upstream connection (hyper HTTP client doesn't support upgrades)
- Client-side key forwarded to upstream for compatibility
- Server uses `auto::Builder::serve_connection_with_upgrades()` instead of `.with_upgrades()`

### Goal
WebSocket connections (including Vite HMR) tunnel through the proxy transparently.

### What to Build

1. **WebSocket detection** (`src/proxy/http.rs`):
   - Check for `Upgrade: websocket` + `Connection: upgrade` headers
   - If detected, delegate to WebSocket handler instead of HTTP forwarder

2. **WebSocket tunnel** (`src/proxy/websocket.rs`):
   - Use `hyper::upgrade::on()` to get client upgraded connection
   - Forward upgrade request to upstream
   - Use `hyper::upgrade::on()` for upstream upgraded connection
   - `tokio::io::copy_bidirectional()` between both `Upgraded` connections
   - Must call `.with_upgrades()` on the HTTP connection builder

3. **Connection builder** (`src/proxy/server.rs`):
   - Add `.with_upgrades()` to the `auto::Builder` so WebSocket upgrades work

4. **Loop detection**:
   - Add `X-Antra-Hops` header to forwarded requests
   - If hops >= 5, return `508 Loop Detected`

### Code to Reference

```
docs/research/https.md       — WebSocket upgrade flow
Cargo.toml                   — Already has hyper with "full" features
```

### Key Code Pattern

```rust
// In http.rs handler, detect upgrade:
if is_websocket_upgrade(&req) {
    return websocket::handle_upgrade(req, state).await;
}

// In websocket.rs:
let client_upgrade = hyper::upgrade::on(&mut req);
let upstream_resp = client.request(upstream_req).await?;
let upstream_upgrade = hyper::upgrade::on(upstream_resp);

// Return 101 to client
let response = Response::builder()
    .status(101)
    .header("upgrade", "websocket")
    .header("connection", "upgrade")
    .body(Empty::new())?;

// Spawn tunnel
tokio::spawn(async move {
    let (client_io, upstream_io) = tokio::try_join!(client_upgrade, upstream_upgrade)?;
    let mut client = TokioIo::new(client_io);
    let mut upstream = TokioIo::new(upstream_io);
    copy_bidirectional(&mut client, &mut upstream).await
});
```

### Acceptance Criteria
- [ ] WebSocket chat app works through the proxy
- [ ] Vite HMR works (if Vite is available to test)
- [ ] `X-Antra-Hops` prevents infinite loops
- [ ] `cargo build` with zero warnings

### Exclusions (DO NOT BUILD)
- ❌ No frame-level inspection
- ❌ No WebSocket compression negotiation
- ❌ No HTTP/2 Extended CONNECT
- ❌ No message filtering or modification

---

## Phase 4 — HTTPS / TLS ✅ DONE

### Verified
- CA generation with `rcgen` (self-signed root CA) — **no `subjectAltName`**, since v0.5.0
- CA stored in `~/.config/antra/ca.pem` and `ca-key.pem` (key permissions 0o600)
- Leaf certificate generation on-demand via SNI
- In-memory cert cache with disk persistence (`~/.config/antra/certs/`)
- TLS server with `tokio-rustls` + `rustls` (ring crypto provider)
- HTTP → HTTPS redirect on port 80 (301 Moved Permanently)
- `antra run` starts HTTPS on port 443 with auto-generated certs
- `antra proxy start` starts HTTPS with cert cache
- `X-Forwarded-Proto: https` for TLS-terminated requests
- `cargo build` with zero errors and zero warnings

### Implementation Notes
- Used `rustls` with `ring` crypto provider (matches rcgen's default)
- `x509-parser` is a **direct** dependency since v0.5.0 (`certs::validate`); the rcgen feature remains for CA reconstruction from disk
- SNI resolver implements `ResolvesServerCert` trait
- Certs are cached in memory after first generation; a cached leaf inside its 45-day renewal window is dropped and re-minted
- Leaf certs are stored on disk for persistence across restarts, under a `LEAF_VERSION` marker that purges them when the format or the signing CA changes

### Exclusions (DO NOT BUILD)
- ❌ No trust store modification (Phase 6)
- ❌ No remote/ACME renewal — leafs are re-minted locally (superseded by v0.5.0, which renews inside a 45-day window)
- ❌ No HTTP/2 ALPN (superseded by v0.4.0, which negotiates H2 to the client)

---

## Phase 5 — Domain Resolution ✅ DONE

### Verified
- `DomainResolver` trait with `register()`, `unregister()`, `status()`
- `LocalhostResolver` — no-op for `.localhost` (browser-native per RFC 6761)
- `HostsResolver` — manages `/etc/hosts` entries for `.test` domains
- `CustomResolver` — validates and manages hosts entries for custom domains
- Atomic hosts file writes (temp + rename)
- `BEGIN/END ANTRA MANAGED HOSTS` markers for safe hosts management
- Domain validation rejects public domains (google.com, github.com, etc.)
- Domain validation rejects bare `localhost` (already resolves)
- `antra run` auto-selects resolver based on domain suffix
- Cleanup removes hosts entries on exit
- `cargo build` with zero errors and zero warnings
- 15/15 unit tests pass

### Implementation Notes
- Shared hosts file logic in `resolver/hosts.rs` (read, write, atomic rename)
- Hosts entries are `127.0.0.1 <domain>` within managed block
- Managed block is created automatically if missing
- `.localhost` is always a no-op (browsers resolve natively per RFC 6761)
- `.test` and custom domains use hosts file management
- Public domain blocklist prevents accidental hijacking

### Exclusions (DO NOT BUILD)
- ❌ No local DNS server
- ❌ No dnsmasq integration
- ❌ No mDNS/Bonjour

---

## Phase 6 — Root CA Trust ✅ DONE

### Verified
- `antra trust` installs CA into OS trust store (with user prompt)
- `antra trust --status` shows correct trust state (installed/not installed)
- `antra trust --remove` removes CA from OS trust store (with user prompt)
- `os-truststore` crate handles cross-platform trust store (macOS keychain, Linux ca-certificates, Windows certutil)
- Handles `NeedsElevation`, `InteractiveAuthRequired`, `StoreToolMissing`, `Unsupported` errors
- `antra doctor` checks actual trust status
- User prompted before any system modification
- `cargo build` with zero errors and zero warnings
- 16/16 unit tests pass

### Implementation Notes
- Used `os-truststore` crate (v0.0.2) for cross-platform trust store abstraction
- Certificate identity derived from SHA-256 of DER bytes (stable, no naming needed)
- `Cert::from_pem()` validates the cert is a CA before installation
- `is_installed()` is idempotent — safe to call multiple times
- `install()` is idempotent — already-installed certs are a no-op
- `Report` enum provides `Installed`, `AlreadyInstalled`, `InstalledNotTrusted` outcomes

### Exclusions (DO NOT BUILD)
- ❌ No Firefox NSS store modification
- ❌ No Java trust store
- ❌ No silent installation

---

## Project Structure

```
antra/
├── AGENT.md              ← YOU ARE HERE
├── NEXT-SESSION.md       ← Current scheduled work — execute on "start next session"
├── README.md             ← User docs (install, commands, env vars, security)
├── ROADMAP.md            ← Feature status (done / now / next / later)
├── Cargo.toml            ← Dependencies
├── docs/
│   ├── architecture.md   ← Module design, data types, flows
│   ├── security.md       ← Threat model, safety rules
│   ├── mvp.md            ← In/out scope, definition of done
│   └── research/
│       ├── domain-resolution.md
│       ├── https.md
│       ├── process-management.md
│       ├── portless.md
│       └── crates.md
└── src/
    ├── main.rs           ← Entry point, tracing setup
    ├── cli/              ← All subcommands (Clap)
    ├── certs/            ← CA + leaf generation, strict validation, versioned rotation
    ├── util/             ← Daemon log (one path, writer, tail/follow), ports, output
    ├── config/           ← antra.toml + global state
    ├── daemon/           ← Background proxy process
    ├── ipc/              ← CLI ↔ daemon communication
    ├── platform/         ← macOS/Linux/Windows abstractions
    ├── process/          ← Child process spawning + signals
    ├── proxy/            ← HTTP/HTTPS/WebSocket proxy
    ├── resolver/         ← Domain → 127.0.0.1 resolution
    ├── routing/          ← Route registry + types
    ├── trust/            ← OS trust store (install/remove/check CA)
    └── util/             ← Port allocation, terminal output
tests/
    ├── cert_strict.rs       ← Strict X.509 rules (no CA SAN, validity, EKU)
    ├── e2e_securetransport.rs ← macOS: Apple's TLS stack vs a live daemon
    └── common/              ← Disposable HOME harness shared by the e2e suites
```

---

## Documentation Reference

| File | When to Read |
|------|-------------|
| `NEXT-SESSION.md` | The current scheduled work — execute it when told to "start next session" |
| `README.md` | User-facing: install, commands, env vars, security policy |
| `ROADMAP.md` | What is done / approved next / future — status column is current |
| `docs/architecture.md` | When implementing modules — data types, flows |
| `docs/security.md` | When touching hosts, trust store, or domains (includes CA versioning/rotation) |
| `docs/mvp.md` | When unsure about scope — what's in/out |
| `docs/research/*.md` | When you need background on a specific area |

---

## After Each Phase

1. `cargo build` — zero errors, zero warnings
2. Manual test — verify the feature works end-to-end
3. Update status table in this file
4. Mark phase as ✅ DONE
5. Update the "Current state" line at the top

---

## Quick Command Reference

```bash
# Build & run
cargo build
cargo run -- <args>

# Test
cargo test

# Clean build
cargo clean && cargo build

# Check without building
cargo check

# Clippy lints
cargo clippy

# Format
cargo fmt
```
