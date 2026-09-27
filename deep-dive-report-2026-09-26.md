# Antra — Deep-dive findings report (Phases 1–7)

Sections D/E were approved and **implemented** (v0.5.0, phases 0–5 of the plan below). The verification that shaped the plan, the corrections to it, the per-phase log and the release notes are in [F](#f-verification-corrections-and-approval-plan-2026-09-26); the full plan is `fix-plan-2026-09-26-ca-trust.md`. Still open: tagging/publishing, the manual Safari/Firefox pass, and the follow-up queue.

Evidence base: full repo read (docs, src, tests, landing), live site probe via embedded browser + curl, and a hands-on adversarial session against the freshly-built v0.4.0 binary in a disposable `HOME` (temp daemon on fallback ports, stopped and cleaned afterward; real `/etc/hosts` and trust store untouched).

---

## A. My understanding of Antra

**What it is.** A native-Rust local development proxy: `antra run --domain myapp.localhost -- pnpm dev` starts a daemon that terminates TLS on :443, mints per-SNI leaf certs from a local CA, and routes `myapp.localhost → 127.0.0.1:<port>` to your process. One command replaces the mkcert+Caddy+`/etc/hosts`+port-sprawl ritual.

**Problem it solves.** Local dev URLs are broken UX: `localhost:5173` isn't a secure context in the ways that matter (cookies, Service Workers, WebCrypto, OAuth redirects), and the workaround stack is worse than the problem. Portless (Vercel Labs, ~11.4k stars, Node/TS + shell-out-to-openssl) proves the demand; `docs/research/portless.md` is an explicit gap analysis — Antra is the Rust-native answer: `rustls`/`rcgen`, no runtime deps, in-memory route table, honest consent model.

**Who it's for.** Individual developers and small teams who want production-shaped URLs on a laptop, in any language ("if it binds a port, Antra can front it"). Deliberately not a platform: no cloud, no accounts, no telemetry, no GUI, no tunnels (`README.md:309-320`).

**What makes it different.** Three commitments held consistently in code:
1. **Consent as architecture** — CA install always prompts (PTY-tested), hosts writes scoped to a managed block, atomic temp+rename everywhere, `--allow-custom-domain` as a warning boundary (MITRE T1553.004 named out loud in README).
2. **Error messages that print the fix** — 502/503/508 bodies with numbered instructions (`proxy/http.rs:46-95`); `doctor` reports fallback ports and the exact trust command.
3. **Conceptual economy** — daemon + cert cache + route table; the "what Antra is not" list is as load-bearing as the feature list.

**What it's becoming.** The boring, trusted default for laptop networking — the layer you install once per machine and forget. Late code (`port_watcher`, route restore, daemon split-brain guards) shows the direction is *long-lived multi-app machine state*, which raises the bar on exactly the trust/cert layer.

**Uncertainties I flagged.** Whether the vision includes team-shared config (`antra.toml` "whole team gets the same setup" implies machine-level state is per-user forever) — the project says local-only, so I've assumed per-machine. Landing-page design identity ("Mudra", mandalas) is intentional Indian-inflected minimalism; I treated it as load-bearing, not decoration.

**Docs vs implementation discrepancies (explicit):**
- `README.md:174` says known public names (`google.com`, `github.com`) are **Rejected**; current code accepts them with `--allow-custom-domain` (only a `tracing::warn` — `resolver/util.rs:41-48`). `docs/security.md:23` states the *new* policy ("approval is a warning boundary, not an allowlist"). README wasn't reconciled.
- `ROADMAP.md` status column is drifting: #10 `--force`, #11 loop detection (508), #18 H2-ALPN, #23 streaming forwarder, #24 WS upgrade timeout are all shipped (run.rs:297-328, http.rs:73-96, https.rs:116, forward.rs:20-21, websocket.rs:140-198) but still marked NEXT/LATER. Cleanup items C1/C2/C3 are done.

---

## B. Current state assessment

**Strong.**
- Proxy hot path: streamed (not buffered) forwarding, dual-stack dial with `localhost` fallback (`forward.rs:11-17`), WS tunnel with bounded handshakes + forwarded auth headers + echoed 101 extensions, hop loop detection, HTTP/2 ALPN to browsers.
- Daemon lifecycle hardening is unusually mature for 24-day-old code: pid-file + socket singleton gates, port-bind before claiming socket, stale-socket/split-brain refusal, managed-route restore gated on owner liveness (`daemon/server.rs:157-322`).
- Process hygiene: process-group signals, SIGTERM→grace→kill, route restore when a replaced backend dies, exit-code propagation.
- Test culture: ~300 test executions green on current main (I re-ran the suite, exit 0); 3-OS CI with a hermetic Windows subset; 13 dated `tests/user-test-*.md` logs — a self-disciplined feedback loop, with findings visibly closed (e.g. doctor fallback-port awareness, install.sh content-type).
- Install story integrity: I read `install.sh` end-to-end as a wary user would — checksum verification, TTY-aware CA consent that never auto-installs headless, PATH hints.

**Weak.**
- **The CA certificate itself is defective** (see C-1 — Safari/SecureTransport class).
- Daemon observability is nil: CLI spawns it with `stdout/stderr → null` (`cli/mod.rs:49-56`), no log file, no `antra logs`. "HTTPS server failed" is a `tracing::error!` that goes nowhere. My own 502/503 curl probes failed *silently* — I could not distinguish proxy bug from TLS bug without `openssl s_client`.
- Status/version drift in ROADMAP + README vs code (costs contributor trust — the file that tells contributors what to work on says `--force` doesn't exist).

**Unfinished.**
- `docs/mvp.md` verification checklist still has unchecked boxes, and **no Safari row at all** — the browser whose support `antra hosts sync` exists for was never in the DoD.
- Firefox untested; Windows runtime CI brand-new; `target/x86_64-pc-windows-*` builds exist but local cross-compile toolchain gap noted in AGENT.md.
- Large shell e2e suites (`e2e_all_features.sh`, `e2e_portless_parity.sh`) aren't wired into CI — rot risk.

**Technically risky.**
- Per-request `hyper_util::client::legacy::Client::builder().build_http()` (`forward.rs:88`): a fresh connection pool per request, so upstream keep-alive never reuses — Vite unbundled dev = hundreds of TCP handshakes per reload. Fine today; painful at monorepo scale.
- Extreme CA validity (1975→4096, rcgen defaults, no override anywhere in `certs/`): beyond the malformed-SAN problem, Apple's 825-day policy conversations will eventually bite; at minimum set `not_before = now` honestly.

**Surprisingly good.** Battle-scar comments encoding *why* (e.g. `ca.rs:66-70` race-corrupted PEM story; `run.rs:216-221` "silently remapping born the classic 503"). Landing-page honesty footnotes as a design pattern. `atomic_write` with chown-back for sudo daemons.

**Missing.** Safari verification. Any daemon-log path. A real 404 page and security headers (`_headers` has one rule; live checks: no HSTS/CSP/X-Frame; unknown paths return 200 homepage). `antra logs`. Site demo of the actual 5-second product moment.

---

## C. Top opportunities (ranked)

### 1. Fix the malformed CA SAN — every Apple-stack client rejects Antra HTTPS today
- **Problem:** The root CA embeds `Subject Alternative Name: DNS:Antra Local CA`. A dNSName with spaces is not a valid DNS name.
- **Evidence (reproduced live, disposable HOME):** `openssl s_client` shows `DNS:Antra Local CA` on the CA (rcgen puts `CertificateParams::new(vec![…])` args into SANs — `ca.rs:31`); macOS `curl --cacert ca.pem https://app.localhost:8443` → `SSL certificate problem: unsupported or invalid name syntax` while `openssl verify` → OK. Isolation test: same-shaped cert pair with openssl-generated CA/leaf (notAfter 4764) → curl 200. It's the SAN syntax, not the dates. SecureTransport fails at chain-parse before trust evaluation — same library Safari uses. Chrome/BoringSSL and OpenSSL tolerate it, which is why every verification so far passed.
- **Why it matters:** Safari support is a *feature* (`antra hosts sync` exists for Safari; `antra hosts --help` frames it that way; the user-test logs repeatedly discuss Safari resolution). Resolution works; TLS doesn't. Safari users get `errSSLBadCert`, macOS curl/system tools likewise. The product's central promise — "HTTPS that just works" — silently fails on Apple's native TLS stack.
- **Proposed direction:** Drop the SAN from the CA (keep CN); add CA-migration so existing installs regenerate the CA + re-trust; regression-test CA SAN absence; add a Safari row to the MVP checklist. Details in D/E.
- **Impact:** enormous (correctness of the core promise on ~half of laptops). **Complexity:** trivial code / small migration. **Risk:** low for new installs; medium for existing (CA swap = trust re-prompt — but unavoidable and honest).
- **Identity:** strengthens it — consent-driven, reversible re-trust is exactly Antra's philosophy.

### 2. Close the browser-verification gap with a strict-parser gate
- **Problem:** The bug class that shipped #1 (permissive verifier passes, strict verifier fails) is invisible to the current test suite and checklist.
- **Evidence:** docs/mvp.md checklist unchecked; every "verified" session in AGENT.md is Chrome-only; `cargo test` is OpenSSL-only.
- **Proposed:** (a) unit test asserting CA/leaf DER parses against strict rules (x509-parser already a dep: assert no SAN on CA, valid dNSName syntax on leaves); (b) a `tests/` e2e that runs `curl --cacert` (SecureTransport) as a *second opinion* alongside openssl on macOS CI; (c) formal Safari+Firefox pass in the checklist.
- **Impact:** high (prevents recurrence). **Complexity:** small. **Risk:** low.

### 3. Daemon observability: log file + `antra logs`
- **Problem:** Daemon tracing output is discarded (`cli/mod.rs:49-56`); failures are undiagnosable without strace.
- **Evidence:** `tracing::error!("HTTPS server failed")` exists but goes to null; user-test 2026-09-06 explicitly requested `antra logs -f`; my own probes hit the wall.
- **Proposed:** daemon writes rolling JSONL/plain log to `~/.config/antra/antra.log` (0600); `antra logs [-f]`; doctor prints last 5 error lines.
- **Impact:** high for support/debugging, prerequisite for trust at scale. **Complexity:** small-medium. **Risk:** low (log size discipline needed).

### 4. Site hygiene for an HTTPS product: security headers + real 404
- **Evidence (live):** no HSTS/CSP/X-Frame-Options/Permissions-Policy; `/nonexistent-page` → 200 with homepage body; `privacy.html`/`terms.html` 308-hop.
- **Proposed:** extend `landing/_headers` (HSTS after confirming full-site HTTPS, CSP allowing fonts.googleapis+plausible, `X-Content-Type-Options` already present, frame-deny) + `404.html`.
- **Impact:** medium (credibility; a cert tool's own site failing TLS-hygiene checks is the screenshot people make). **Complexity:** trivial. **Risk:** trivial.

### 5. Upstream connection reuse (shared hyper client)
- **Evidence:** `forward.rs:88` builds a new client per request; MVP excludes H2-to-upstream, but H1 keep-alive pooling is not excluded.
- **Impact:** medium (dev-server reload latency; grows with monorepo/LATER features). **Complexity:** medium. **Risk:** medium (idle-pool vs process-group cleanup interaction).

### 6. Reconcile ROADMAP/README status drift
- Cheap honesty pass: mark shipped items DONE (#10, #11, #18, #23, #24, C1–C3), reword the README public-names row to match security.md policy. **Impact:** medium for contributors (their entry point lies to them). **Complexity:** trivial. **Risk:** none.

### 7. Wrong-subcommand hint in hosts permission errors
- **Evidence:** `resolver/hosts.rs:55` prints `Re-run with: sudo antra alias {domain} <port>` — observed verbatim while running `antra run --domain google.com --allow-custom-domain`. **Trivial, isolated correctness.**

### 8. Landing: make the product moment move
- The terminal demo block is static text; the actual product *is* a 5-second ritual. An asciinema-style embedded recording (or CSS typing animation in Mudra's motion grammar — slow, faint, purposeful) converts better than any comparison table. Also: mobile nav (<768px) drops GitHub/Docs links with no substitute (verified via computed style). **Impact:** medium (stars are the project's oxygen — 0 today, repo is 24 days old). **Complexity:** small-medium. **Risk:** none (stay inside Mudra conventions: hairlines, mono labels, reduced-motion honored).

### 9. Domain-shape laxness: underscores accepted (`my_app.test`)
- `validate_domain_shape` allows `_`; SAN dNSName with underscore is invalid DNS — the same failure family as #1 at smaller scale. Reject or normalize with warning. **Small but consistent with #1.**

---

## D. The one recommended contribution

**Fix the CA SAN defect (#1), delivered together with the strict-parser regression gate (#2).**

Why this over everything else:
- It is the only finding that makes the *advertised core promise* false ("HTTPS that just works… for Safari compatibility we sync hosts" — but Safari can't complete the handshake). Everything else is polish, speed, or hygiene; this is correctness.
- It sits at the project's deepest identity point: Antra sells *honest security*. A malformed root cert that only strict validators reject is the one embarrassment this product must not carry to adoption.
- Highest leverage per unit of effort: one line in `ca.rs` + migration + tests, versus, say, #5 (perf work that only matters at a scale the project hasn't reached).
- It is also the highest-value contribution *as an outsider* — it's the kind of bug a newcomer finds in week one and the kind the maintainer can't see (macOS + Chrome-only verification is the maintainer's exact blind spot, and AGENT.md even lists "Firefox not tested" as a known gap).

## E. Implementation plan (for D)

**Exact files**
1. `src/certs/ca.rs:31` — `CertificateParams::new(vec![])` (no SANs on a CA; CN stays via `distinguished_name`). Optionally set `params.not_before = not_before_utc()/now` honestly (rcgen 0.14 supports it) while keeping long `not_after`.
2. `src/certs/store.rs` — CA migration gate: add `CA_VERSION` marker (mirror the existing `ensure_leaf_version` pattern at `store.rs:158-177`). On `get_or_create_ca`: if marker missing/mismatched, parse existing `ca.pem`; if its SAN is non-empty (or marker absent on a legacy CA), regenerate CA **and** purge cached leafs (bump `LEAF_VERSION` to "3" in the same release so stale leaves can't be served from the old CA).
3. `src/trust.rs` — after CA swap: detect old-CA-installed (`is_installed` is already idempotent) → prompt to remove old + install new in one flow; `--yes` honored; never silent (security.md rules).
4. `src/cli/doctor.rs` — new check: "CA certificate passes strict X.509 validation" (parse with `x509-parser`, assert SAN empty + dNSNames valid on leaves) with fix hint `antra trust --remove && antra trust`.
5. `src/resolver/hosts.rs:55` — thread the invoking subcommand into the hint (one extra `&str` param; callers in `cli/run.rs`, `cli/alias.rs`, `cli/add.rs`, `cli/proxy.rs`).
6. **Tests:** `tests/cert_store.rs` — CA has zero SAN entries; leaf SAN is a syntactically valid DNS name; legacy-CA marker triggers regeneration + leaf purge (tempdir, no trust-store writes). `tests/e2e_adversarial.rs` — macOS-only test hitting the live daemon with `curl --cacert` (SecureTransport second opinion) asserting 200/502/503 bodies. `docs/mvp.md` — add the missing Safari row; check Chrome/Safari/Firefox boxes only after the manual pass (AGENT.md: Firefox pending).

**Architectural implications:** none — CA is already generated in one place and consumed via `Issuer`; the marker pattern already exists. Migration must run in both CLI (`run/dev/alias`) and daemon-start paths so no process serves a legacy CA.

**Backward compat:** users who ran any Antra ≤0.4.0 keep a defective CA until migration; the marker+doctor detection handles it. Existing system trust entries for the old CA *cannot* be silently removed on all platforms (macOS GUI auth) — the prompt path must explain "remove old, add new". Windows/Linux `--yes` paths work headless.

**UX implications:** one-time re-trust prompt after upgrade (clearly worded, reversible — consistent with existing consent copy). `antra list`/routes unaffected.

**Risks:** (a) CA regeneration invalidates pinned `NODE_EXTRA_CA_CERTS` consumers until new CA written — handled by doing swap before daemon binds; (b) if any user exported `ca.pem` elsewhere (team scripts), they must re-export — release-note it; (c) migration test needs the hermetic-HOME discipline AGENT.md prescribes.

**Verification I can do here:** full suite + clippy + fmt; live temp-daemon `curl --cacert` on macOS (the exact repro failing today must return 200 after the fix). **I cannot verify:** real Safari/Firefox windows (no GUI here) — that stays on the checklist for you, and the curl-SecureTransport probe is its proxy.

**Out of scope for this change:** #3 logs, #4 headers, #5 pooling — propose as follow-ups.

---

**What I did not do:** no files edited, no commits, no trust-store or hosts changes, no `antra clean` against real state. All daemon/cert artifacts lived in `/tmp/antra-adv.*`/`/tmp/antra-test.*` and were stopped+removed. The embedded-browser visual pass (mandalas/reveals) couldn't render — viewport reported 0×0 — so design findings are from source + computed-style probes, and site findings are curl-verified.

## F. Verification, corrections and approval plan (2026-09-26)

D/E were approved. Before writing code I re-derived the central claim from scratch, because the whole contribution rests on it. Result: the claim holds, the mechanism is confirmed in the dependency source, and the plan needs five corrections plus four risks section E did not see.

### F.1 Independently reproduced

| Claim | Verdict | How |
|---|---|---|
| CA carries `Subject Alternative Name: DNS:Antra Local CA` | Confirmed | generated a CA through the current lib, `openssl x509 -noout -text` |
| Permissive validator accepts, strict one rejects | Confirmed | `openssl verify -CAfile ca.pem leaf.pem` → `OK`; macOS `/usr/bin/curl` (SecureTransport — the library Safari uses) → `curl: (60) SSL certificate problem: unsupported or invalid name syntax` |
| The SAN is the cause, not dates/keys/signatures | Confirmed by control | same-shaped openssl CA+leaf, CA with **no** SAN, 10-year leaf → SecureTransport returns **200** |
| Mechanism | Confirmed in rcgen 0.14.10 | `CertificateParams::new(v)` maps each string to `SanType::DnsName` (`certificate.rs:111-126`); `write_subject_alt_names` returns early when the list is empty (`certificate.rs:288-289`); `DistinguishedName::push` is insert-or-update (`lib.rs:508-513`), so `CertificateParams::default()` yields one clean CN |
| Leaf shape is already correct | Confirmed | SAN `DNS:app.localhost`, EKU serverAuth, AKI present, `ecdsa-with-SHA256` |
| Report §C-7 wrong subcommand hint, §C-6 roadmap drift, §B "per-request client" | Confirmed | `resolver/hosts.rs:55`; `--force` `cli/run.rs:42,297`, 508 `proxy/http.rs:74-93`, ALPN `proxy/https.rs:116`; client per request `proxy/forward.rs:88-89` |

Probe artifacts (`/tmp/antra-probe`, two throwaway test files, three `openssl s_server` instances) were removed/stopped; the real `~/Library/Application Support/antra` was verified byte-identical afterwards.

### F.2 Corrections to E

1. **`x509-parser` is not a dependency.** It is an rcgen *feature* (`Cargo.toml:30`), i.e. transitive. Using it directly needs a new direct dependency, pinned to `0.16` to share rcgen's copy. Section C-2(a)'s "already a dep" is wrong.
2. **Leaf certificates carry the same 1975→4096 window as the CA** (verified in the dump), and Apple's 825-day limit applies to custom roots: mkcert's own source comment says certificates last "2 years and 3 months, which is always less than 825 days, the limit that macOS/iOS apply to all certificates, including custom roots" (mkcert `cert.go`, citing support.apple.com/en-us/HT210176). Section B's "will eventually bite" understates it: fixing only the SAN can still leave Safari rejecting a 4096-dated leaf, so the Safari promise stays conditional. **Shipping the validity bound in the same release.**
3. **Leaf purging needs no new plumbing.** `LEAF_VERSION` is enforced in `CertStore::new()` (`certs/store.rs:28`), not in `get_or_create_leaf`, and rotation flows through that same constructor — bumping the constant is sufficient to drop every leaf signed by a retired CA.
4. **Trust removal can be exact, not best-effort-by-name.** macOS removes by SHA-1 hash of the exact PEM payload (`trust.rs:117-192,788-807`), Windows CurrentUser by DER compare (`trust.rs:498-523`), the system store by os-truststore's SHA-256-of-DER identity. E's worry that "existing entries cannot be silently removed" is a permissions problem, not an identification problem.
5. **Re-trust after rotation needs no elevation on macOS.** `install_ca_noninteractive` is login-keychain only (`trust.rs:403-431`), so the post-rotation flow is silent, sudo-free, and GUI-free.

### F.3 Risks E missed

- **The test suite is not hermetic, and rotation makes that dangerous.** `tests/e2e_*.rs` spawn the real binary with the inherited environment; hermeticity is a manual wrapper in `AGENT.md:54-56`, and `tests/user-test-2026-09-06-021.md:168-170` already records a `cargo test` wiping a real CA. Once `get_or_create_ca` can rotate a CA, an unhermetic test run silently rotates the developer's CA and desyncs their keychain. Verified fix: with `HOME`+`XDG_CONFIG_HOME` pointed at a temp dir, `antra trust --status` saw no CA at all and the real config dir stayed byte-identical. **This is Phase 0 and it blocks the rotation code.**
- **`was_trust_prompted()` short-circuits the re-prompt.** `cli/run.rs:128` returns before the trust check at `:132`, so after a CA swap the user is never asked to re-trust: HTTPS broken, terminal green. The flag lives in `config/global.rs:44-52` and must be re-armed on rotation (or the checks inverted).
- **A daemon from the old binary keeps serving the retired CA**, because `CertCache` holds it for the process lifetime (`certs/cache.rs:13-43`). Needs a CA fingerprint on `IpcPayload::Status` (`ipc/protocol.rs:92-98`) so `doctor` can say "restart the daemon".
- **Constructing a "now"-based validity needs `time` as a direct dependency** — `rcgen::date_time_ymd` is exported but takes a hardcoded calendar date, which is not what a validity window needs.

### F.4 Approved plan (phases, each with an exit gate)

Decisions taken: bound validity in this release; CA and leaf at mkcert parity (2y3m, under 825 days); silent renewal at <45 days remaining, logged; rotation automatic on disk, trust-store cleanup byte-exact and reported, never silent.

| Phase | Change | Exit gate |
|---|---|---|
| 0 | Hermetic test HOME: `tests/common` helper applied to every e2e spawn (`HOME`, `XDG_*`, `TMPDIR`, `APPDATA`, `LOCALAPPDATA`) | `cargo test` leaves the real config dir and keychain byte-identical |
| 1 | Strict X.509 gate: direct `x509-parser = "0.16"`, new `src/certs/validate.rs` (`check_ca`, `check_leaf`, `remaining_days`), `tests/cert_strict.rs` | the test **fails today**, naming the observed SAN — that is the recorded before-state |
| 2 | CA v2 (no SAN, CN only, bounded validity) and leaf v3 (same window, `LEAF_VERSION` → `"3"`, regenerate in `CertCache` when <45 days remain) | `cert_strict` green, full suite green, clippy/fmt clean |
| 3 | Rotation: `CA_VERSION` marker + `rotated-ca.pem` in `get_or_create_ca` (atomic, crash-safe), byte-exact retired-CA removal after the new CA is trusted, re-prompt ordering fix, `ca_fingerprint` on `StatusResponse` + `doctor` mismatch warning, `PROTOCOL_VERSION` 2→3 | rotation tests in tempdirs (no trust-store writes) + a manual temp-`HOME` walkthrough |
| 4 | `doctor`: strict-CA check and trusted-vs-disk CA check; reconcile `README.md:174`, ROADMAP #10/#11/#18/#23/#24/C1–C3, `docs/mvp.md` Safari row, `docs/security.md` rotation section; 0.5.0 release notes | no remaining doc/code contradiction (grep-checked) |
| 5 | `tests/e2e_securetransport.rs`: macOS-gated, in-process rustls server + `/usr/bin/curl --cacert` → 200, plus a variant through a live `antra proxy start` + `antra alias` under the Phase-0 home | the exact command that returns `curl: (60)` today returns 200; promoted to macOS CI |

Follow-ups stay separate PRs, in order: daemon log file + `antra logs` (C-3), the `sudo antra alias` hint (C-7), underscore domains (C-9), landing headers/404 (C-4), wiring the shell e2e suites into CI, then the shared upstream client (C-5). `antra logs` and the shared client are the two that need design decisions of their own; everything else is mechanical once the CA work lands.
>
> **Update (2026-09-27, v0.6.0):** C-3, C-7, C-9, C-4 and C-5 all shipped in [PR #14](https://github.com/ifelse-codes/antra/pull/14). `antra logs` also fixed a path mismatch that had made the launchd service's log unreachable from the CLI, and the shared client is proved by a test that counts upstream TCP connections (4 requests → 1 connection, where the old per-request build gave 4). Still open: the manual Safari/Firefox pass, and wiring the shell e2e suites into CI — blocked on the auto-started daemon having no port override (ROADMAP #21), since those suites need 8443 free.

Not verifiable here and left on the checklist: real Safari and Firefox passes, and whether Apple's 825-day limit is enforced against a *user-keychain* anchor (my SecureTransport probe accepted a 10-year leaf under a `--cacert` anchor, so it is weak evidence either way — mkcert parity is the safe default).

### F.5 Outcome (implemented, v0.5.0)

Phases 0–5 shipped. The evidence that each gate actually ran:

| Claim | Evidence |
|---|---|
| The malformed root is gone | `openssl x509` on a freshly generated CA: `subject=CN=Antra Local CA`, `notAfter` 800 days out, **no SAN extension**. A test re-mints the old shape and asserts it is *still* rejected, so the gate is not vacuous. |
| The suite can no longer pass on a permissive verifier | `tests/cert_strict.rs` went red before the fix — `CA certificate must carry no subjectAltName, found: DNS:Antra Local CA`, `CA must expire within the 825-day limit, got 755783 days` — and green after. |
| Apple's stack accepts the chain, in production shape | `tests/e2e_securetransport.rs`: `/usr/bin/curl --cacert` (SecureTransport) returns 200 through a real `antra proxy start` + `antra alias`, and still refuses a chain from an untrusted CA. |
| Existing installs migrate and the old root stops being trusted | Manual walkthrough in a disposable HOME: deleting `.ca-version` (what a ≤0.4.0 install looks like) makes the next command rotate the CA, write `retired-ca.pem`, and purge the leaf cache; `doctor` then reports *"CA passes strict X.509 validation (expires in 800d)"* plus *"A superseded CA is still present in a trust store"*; `trust --status` names the fix. |
| `cargo test` cannot touch the developer's machine | The e2e suites now spawn against a disposable HOME. Verified by `shasum` on the real `ca.pem`/`ca-key.pem` and a keychain cert count before and after a full run: unchanged. |

Two things the plan did not anticipate, both now fixed and covered by tests: the disposable home has to live under a short root (macOS has no XDG runtime dir, so the daemon's socket lands under `$HOME/Library/Application Support/antra/` and blew the 104-byte `sun_path` limit from a deep temp root), and the suite has to stop its daemon on exit or the leftover process holds 8443 and the developer's next `antra run` cannot bind.

Not fixed here, deliberately: the follow-up queue in `fix-plan-2026-09-26-ca-trust.md` §Phase 6. ROADMAP #32 (`antra logs`) and #33 (shared upstream client) shipped in **v0.6.0**; the sudo hint, underscore domains and landing headers/404 went with them.
