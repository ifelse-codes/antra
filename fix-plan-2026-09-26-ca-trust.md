# Fix plan — CA trust correctness (target: v0.5.0)

Date: 2026-09-26 · Scope: the defect + gaps in `deep-dive-report-2026-09-26.md` §C/§D/§E, then the follow-up queue.
Status: **phases 0–5 implemented** (see §9). Phases 4 (docs) and 5 (SecureTransport e2e) are in; tagging and publishing are still yours.

---

## 1. Verdict on the report

### 1.1 Independently reproduced (so the premise is not taken on faith)

| Claim | Verdict | How |
|---|---|---|
| CA embeds `Subject Alternative Name: DNS:Antra Local CA` | **Confirmed** | generated a CA via the current lib, `openssl x509 -noout -text` → `X509v3 Subject Alternative Name: DNS:Antra Local CA` |
| Permissive validators accept, strict ones reject | **Confirmed** | `openssl verify -CAfile ca.pem leaf.pem` → `OK`, while macOS `/usr/bin/curl` (SecureTransport, the library Safari uses) → `curl: (60) SSL certificate problem: unsupported or invalid name syntax` |
| The SAN is the cause (not dates, not key/sig) | **Confirmed by control** | same-shaped openssl CA+leaf pair, CA with **no** SAN, 10-year leaf → SecureTransport curl returns **200** |
| Mechanism | **Confirmed in rcgen 0.14.10 source** | `CertificateParams::new(v)` maps each string to `SanType::DnsName` (`certificate.rs:111-126`); `write_subject_alt_names` returns early when empty (`certificate.rs:288-289`) |
| Leaf has SAN + EKU serverAuth + AKI, chains to CA | **Confirmed** | leaf text: SAN `DNS:app.localhost`, `TLS Web Server Authentication`, AKI present, `ecdsa-with-SHA256` |
| `hosts.rs:55` prints a hardcoded `sudo antra alias …` hint | **Confirmed** | `write_hosts_with_hint` builds the message with no knowledge of the invoking subcommand |
| ROADMAP drift (#10, #11, #18, #23, #24 shipped but marked NEXT/LATER; C1–C3 done) | **Confirmed** | `--force` `cli/run.rs:42,297`; 508 `proxy/http.rs:74-93`; H2 ALPN `proxy/https.rs:116`; streaming forward `proxy/forward.rs:22-114`; WS upgrade timeout in `proxy/websocket.rs`; `Commands2`/`#![allow(dead_code)]` gone; socket perms `0o600` `daemon/server.rs:249` |
| README:174 "known public names Rejected" vs code accepting with `--allow-custom-domain` | **Confirmed** | `README.md:174` vs `resolver/util.rs:31-50` and `docs/security.md:23` |
| Per-request client build kills upstream keep-alive | **Confirmed** | `proxy/forward.rs:88-89` builds a new `Client` per request |

### 1.2 Corrections to the report

1. **`x509-parser` is not a dependency.** It is an *rcgen feature* (`Cargo.toml:30`), i.e. a transitive crate. `use x509_parser::…` needs a new direct dependency. Pin `0.16` to share one copy with rcgen (0.18 is already in the local registry from another project — do not unify on it).
2. **Leaf certificates have the same 1975→4096 window as the CA** (verified in the dump), and Apple requires TLS *server* certs to be ≤825 days (see §1.3). The report mentions this only as a passing "Apple's 825-day policy conversations".
3. **`LEAF_VERSION` purging happens in `CertStore::new()`** (`certs/store.rs:28`), not in `get_or_create_leaf`. So bumping `LEAF_VERSION` is sufficient to drop every leaf signed by a retired CA — the ordering already works, no new plumbing.
4. **Removal paths are already byte-exact** on all three platforms: macOS login keychain by SHA-1 hash of the exact PEM payload (`trust.rs:117-192, 788-807`), Windows CurrentUser by DER compare (`trust.rs:498-523`), system store by os-truststore's SHA-256-of-DER identity. Retiring a CA can therefore be done precisely, not by name.
5. **`install_ca_noninteractive` on macOS is login-keychain only, never the system store** (`trust.rs:403-431`), so the re-trust path after a CA swap needs no elevation and no GUI auth prompt.

### 1.3 Gaps the report missed (these are the ones that bite)

- **The test suite is not hermetic, and CA rotation would make that dangerous.** `tests/e2e_*.rs` spawn the real binary with the inherited environment; hermeticity is a manual shell wrapper in `AGENT.md:54-56` (`HOME=$(mktemp -d) cargo test`). `tests/user-test-2026-09-06-021.md:168-170` already records a `cargo test` wiping the developer's real CA. Once `get_or_create_ca` can *rotate* a CA, an unhermetic test run silently rotates the developer's CA and desyncs their keychain. **This must be fixed first, before any rotation code lands.** (I verified the redirect works: with `HOME`+`XDG_CONFIG_HOME` pointed at a temp dir, `antra trust --status` saw no CA at all and the real `~/Library/Application Support/antra` was byte-identical afterwards.)
- **`was_trust_prompted()` short-circuits the re-prompt.** `cli/run.rs:128` returns early when the flag is set, *before* the `is_trusted_for_https()` check at `:132`. After a CA swap the flag is still true, so the user is never asked to re-trust: HTTPS is silently broken with a green terminal. The flag is in `config/global.rs:44-52`. Ordering must invert, or the flag must be re-armed on rotation.
- **A daemon left running from the old binary keeps serving the retired CA.** `CertCache` holds the CA in memory for the process lifetime (`certs/cache.rs:13-43`); the CLI's rotation cannot reach it. `IpcPayload::Status` (`ipc/protocol.rs:92-98`) is the natural place for a CA fingerprint so `doctor` can say "daemon is serving a retired CA — restart it".
- **Apple's 825-day limit applies to custom roots, per the reference implementation.** mkcert's own source comment: *"Certificates last for 2 years and 3 months, which is always less than 825 days, the limit that macOS/iOS apply to all certificates, including custom roots"* (mkcert `cert.go`, citing support.apple.com/en-us/HT210176). Apple's doc states TLS server certs must be ≤825 days and that violating connections "will fail … in Safari in iOS 13 and macOS 10.15". So fixing only the SAN would still leave a 4096-dated leaf that Safari may reject — the Safari promise stays broken. Ship the validity bound in the same release.
- **My SecureTransport probe is weak evidence about 825 days** (a 10-year leaf was accepted with `--cacert`, a custom anchor). Anchored trust evaluation may differ from user-keychain trust. mkcert parity is the safe default; real-Safari confirmation goes on the checklist.

---

## 2. Root cause, precisely

`src/certs/ca.rs:31`

```rust
let mut params = CertificateParams::new(vec!["Antra Local CA".to_string()])?;
```

`CertificateParams::new` treats each argument as a *subject alternative name*, so the root carries `dNSName = "Antra Local CA"`. A `dNSName` must be a syntactically valid DNS name; a value with spaces is not. Chain evaluation that stops at a name-parse step (Apple's SecureTransport, and therefore Safari and every macOS system tool using it) fails before it ever evaluates trust, so the certificate is rejected no matter that it is installed and trusted. OpenSSL and BoringSSL tolerate it, which is exactly why every verification so far passed.

The CN is already set separately via `distinguished_name.push` (`ca.rs:33-35`) and stays.

---

## 3. Target state

**CA v2**
- No SAN extension at all (a CA is identified by its subject, never by name resolution).
- Subject `CN=Antra Local CA` only. `DistinguishedName::push` is insert-or-update (rcgen `lib.rs:508-513`), so switching to `CertificateParams::default()` yields a single clean CN — verified against the current dump, which also shows one CN.
- `notBefore = now − 1h`, `notAfter = now + 2y3m` (mkcert parity, ~820 days, under Apple's 825).

**Leaf v3**
- `subjectAltName = DNS:<hostname>` (unchanged), CN = hostname, AKI, EKU serverAuth, `ecdsa-with-SHA256` (unchanged).
- `notBefore = now − 1h`, `notAfter = now + 2y3m`, regenerated automatically when <45 days remain.

**Rotation contract**
- One funnel: `CertStore::get_or_create_ca` (`certs/store.rs:80-89`), reached by `certs/cache.rs:34` (daemon) and `trust.rs:249,414,438,559` (CLI). Rotation happens before any TLS listener binds, so no process ever serves a retired CA except an already-running old daemon (detected, see Phase 3).
- The retired CA is written to `~/.config/antra/rotated-ca.pem` before the new one replaces it, so an interrupted rotation is recoverable on the next run.
- The retired CA is removed from every trust store it is in, byte-exactly, never silently, and only *after* the new one is trusted.

**UX contract**
- One re-trust prompt after upgrade, worded as a rotation (not a first run), reversible, `--yes`/`--no-trust-prompt` honoured. `docs/security.md:47-56` (always prompt, explain, provide undo) stays satisfied.

---

## 4. Work plan

Each phase has an exit gate. Do not start a phase before the previous gate passes.

### Phase 0 — Hermetic test harness (blocker; no product behaviour change)

Why first: it is the only thing standing between a rotation and a developer's real keychain.

- New `tests/common/mod.rs`: `TestHome` (a `TempDir` + env map) and `fn antra_cmd(home: &TestHome) -> Command` that applies `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_RUNTIME_DIR`, `TMPDIR` (linux/macOS) and `APPDATA`, `LOCALAPPDATA` (Windows) — covering every path `platform::config_dir()` and `platform::ipc_path()` resolve (`platform/mod.rs:6-45`).
- Route all spawns in `tests/e2e_adversarial.rs`, `tests/e2e_binary.rs` through it (two duplicated spawn helpers today: `:19-51` and `:57-91`).
- Because each e2e test now needs a lifetime-owned temp dir, the spawn helpers take `&TestHome`.

Tests: one unit test asserting the env map points `dirs` at the temp dir; the pre-existing e2e assertions stay unchanged.

Gate: `cargo test` leaves `~/Library/Application Support/antra` and the login keychain byte-identical (record `shasum` of `ca.pem`/`ca-key.pem` and `security find-certificate -a -c "Antra Local CA" -Z` before and after). Also run once with the AGENT.md wrapper to prove the two paths agree.

### Phase 1 — Strict X.509 gate (must fail before the fix)

- `Cargo.toml`: add `x509-parser = "0.16"` (regular dependency, not dev-only — Phase 3 needs validity dates at runtime).
- New `src/certs/validate.rs`:
  - `check_ca(der) -> Result<()>` — CA must have **no** SAN extension; `basicConstraints` CA:TRUE; validity window ≤825 days and `notBefore` ≤ now.
  - `check_leaf(der, hostname) -> Result<()>` — every SAN is a syntactically valid `dNSName` equal to the hostname; EKU contains serverAuth; window ≤825 days; `notBefore` ≤ now.
  - `remaining_days(der) -> Result<i64>` — reused by Phase 3 renewal.
- `src/certs/ca.rs:30` and `src/certs/leaf.rs:33` call these on generation and log/return a warning on violation (a generator that cannot produce a strict cert is a bug, not a warning — decide: `debug_assert` in debug, `tracing::warn!` in release, hard error in the strict test).

Tests (`tests/cert_strict.rs`): the four assertions above against freshly generated CA and leaf.

Gate: `cargo test --test cert_strict` **fails today** on `ca_has_no_subject_alt_name` with a message naming the observed SAN. Record that output as the before-state.

### Phase 2 — Generate CA v2 / leaf v3

- `src/certs/ca.rs:30-48`: `CertificateParams::default()` instead of `new(vec!["Antra Local CA"])`; keep the CN push; set explicit `not_before`/`not_after`. Comment must state *why* (a `dNSName` must be a valid DNS name) and cite the SecureTransport failure mode, so nobody "helpfully" adds the SAN back.
- `src/certs/leaf.rs:33-52`: same validity window. Comment cites Apple's 825-day rule for TLS server certs.
- `Cargo.toml`: add `time = "0.3"` (already in the tree via rcgen) to build `OffsetDateTime::now_utc()`; `rcgen::date_time_ymd` is exported but needs a hardcoded calendar date, which is not what we want.
- `certs/store.rs:12`: `LEAF_VERSION` `"2"` → `"3"` (window change + new CA invalidates every cached leaf; `CertStore::new` purges automatically).
- `certs/cache.rs:60`: if the cached/disk leaf has <45 days left, regenerate and replace the in-memory entry (needed because the window is now bounded).

Gate: `cargo test --test cert_strict` green; `cargo test` green; `cargo clippy --all-targets -- -D warnings`; `cargo fmt --check`.

### Phase 3 — Rotation machinery

- `certs/store.rs`:
  - `const CA_VERSION: &str = "2"` + `~/.config/antra/.ca-version` marker, mirroring `ensure_leaf_version` (`store.rs:161-177`) — same read-marker/act/write-marker shape, same rationale comment.
  - `get_or_create_ca` (`store.rs:80-89`): CA present + marker ≠ current → copy current `ca.pem` to `rotated-ca.pem`, generate + atomically save the new CA, write marker, `tracing::info!`/`warn!`. Return type unchanged (no churn at the 5 call sites); expose `pub fn pending_retired_ca_pem(&self) -> Option<String>` for the trust side.
  - Marker write must happen *after* the CA save, and `save_ca` is already atomic (`ca.rs:72-88`), so a crash mid-rotation re-runs cleanly.
- `cli/run.rs:111-179` (`maybe_prompt_trust`): invert the order — check `is_trusted_for_https()` before the `was_trust_prompted()` short-circuit; when the flag is set but trust is false (i.e. a rotation happened), prompt again with a rotation-specific message ("Antra regenerated its local CA to fix a certificate issue; the old one is no longer trusted") and re-arm the flag. `os_truststore` identity is SHA-256-of-DER, so after a swap both `check_trust_status` and `check_user_level_trust` correctly report untrusted — no false green here (unlike the presence-based check the 2026-09-06-1745 user test hit; that path is already byte-based).
- `trust.rs`: after any successful install (`:249`, `:414`, `:438`, `:559` paths), if `rotated-ca.pem` exists → `remove_ca_exact(&old_cert)`; on success delete the file and print what was removed; on failure keep the file and report the exact retry command. Never swallow the failure — a stale trusted root is the thing this whole change exists to prevent.
- `ipc/protocol.rs`: `StatusResponse` gains `#[serde(default)] ca_fingerprint: Option<String>` (SHA-256 of CA DER, hex, first 16 chars); daemon fills it in. `PROTOCOL_VERSION` 2→3; `#[serde(default)]` keeps an old daemon's reply parseable.
- `cli/doctor.rs`: if a daemon is running and its fingerprint ≠ on-disk CA → warning "Daemon is serving a retired CA — `antra proxy stop && antra proxy start`", exit 1.
- `cli/clean.rs`: nothing to change — `remove_local_state` deletes the whole config dir (`clean.rs:148-153`), markers and `rotated-ca.pem` included.

Tests (`tests/cert_store.rs` additions, all `TempDir`, no trust-store writes): marker absent + legacy CA on disk → rotation happened, CA bytes changed, `rotated-ca.pem` holds the old PEM, all `*.pem` leaves purged; marker current → no rotation, CA bytes stable across two `get_or_create_ca` calls; interrupted rotation (marker absent, `rotated-ca.pem` already written) → still converges; leaf regenerated when <45 days remain.

Gate: full suite green; a manual end-to-end in a temp `HOME` (Phase 0 helper) that walks legacy-CA → rotate → prompt → re-trust → `curl` 200.

### Phase 4 — Doctor, docs, release

- `cli/doctor.rs`: new check "CA passes strict X.509 validation" (calls `certs::validate`) with fix hint `antra trust --remove && antra trust`; new check "trusted CA matches the CA on disk" surfacing rotation state.
- `README.md:174`: replace the "Known public names → Rejected" row with the actual policy from `docs/security.md:23` (warning boundary, requires `--allow-custom-domain`).
- `ROADMAP.md`: #10, #11, #18, #23, #24 → **DONE** (cite the code anchors in the notes column); C1, C2, C3 → **DONE**; add a row for CA v2 / rotation; the status legend at `:4` should gain a `DONE` definition (it currently has none).
- `docs/mvp.md:78-88`: add the missing Safari row; leave Firefox unchecked until a human runs it; note that the checkbox is only meaningful *after* `antra doctor` reports a strict-valid CA.
- `docs/security.md`: new "CA rotation" subsection — what regenerates it (version marker, expiry window), what is removed from trust stores, how to undo (`antra trust --remove`).
- `docs/architecture.md`: update the cert section (CA version marker, rotation, bounded validity) if it describes the current flow.
- Release `0.5.0`: `Cargo.toml` version, `Formula/` pin, `install.sh` pin, release notes stating: the CA is regenerated once; you will be asked to re-trust; `NODE_EXTRA_CA_CERTS` consumers keep the same path but must re-read `ca.pem` (its contents change); anyone who exported `ca.pem` elsewhere must re-export.

Gate: `antra doctor` on a legacy install prints the strict check and the rotation state; docs contain no remaining contradiction with code (grep `Rejected` / `NEXT` / `LATER` against the code anchors).

### Phase 5 — Strict-parser end-to-end (the actual regression gate)

- New `tests/e2e_securetransport.rs`, `#[cfg(target_os = "macos")]`, skipped when `/usr/bin/curl` is missing:
  - In-process rustls server using a generated CA+leaf for `app.localhost` on an ephemeral port, serving `200`.
  - `/usr/bin/curl --cacert <ca> --resolve app.localhost:<port>:127.0.0.1 https://app.localhost:<port>/` → assert exit 0 and `http_code == 200`. This is byte-for-byte the command that returns `curl: (60) … unsupported or invalid name syntax` today.
  - Second layer: spawn `antra proxy start --port <free> --http-port <free>` and `antra alias` under the Phase-0 temp `HOME`, then the same curl → asserts the whole chain (daemon → CertCache → SNI → upstream).
- Optional Linux equivalent with `curl --cacert` (OpenSSL) as a cheap cross-platform guard; the macOS one is the one that carries the Safari signal.

Gate: macOS CI (and a local run) green; the pre-fix binary fails this test — keep the failure output in the PR description.

### Phase 6 — Follow-ups (separate PRs, ordered)

| # | Item | Anchor | Acceptance |
|---|---|---|---|
| P1.1 | Daemon log file + `antra logs [-f]`; `doctor` tails the last errors | daemon stdout/stderr are `/dev/null` at `cli/mod.rs:54-55`; `tracing_subscriber` only initialised in `main.rs:30` so the daemon's output goes nowhere | failing HTTPS bind is diagnosable from `antra logs`; log is 0600 and size-capped |
| P1.2 | Hosts hint names the invoking subcommand | `resolver/hosts.rs:55` | `antra run …` prints a `run`-shaped hint; update the 4 call sites |
| P1.3 | Reject/normalise `_` in domain shape | `resolver/util.rs:73-108` allows `_`, which is an invalid `dNSName` — the same failure family as §2, smaller | `my_app.test` is rejected with a clear message; leaf SAN validity test covers the accepted alphabet |
| P1.4 | Landing hygiene: `404.html`, HSTS/CSP/X-Frame-Options/Permissions-Policy | `landing/_headers` has one rule | unknown path returns 404; `curl -I` shows the headers |
| P1.5 | Wire `e2e_all_features.sh` / `e2e_portless_parity.sh` into CI, or delete them | not referenced by `.github/workflows/ci.yml` | no rotting suites |
| P2.1 | Shared upstream HTTP client | `proxy/forward.rs:88-89` builds a client per request | a keep-alive reuse test (`tests/upstream_dual_stack.rs` pattern) shows one upstream connection for N requests; idle pool closed on shutdown, no interaction with process-group cleanup |
| P2.2 | Landing product-moment demo + mobile nav links | `landing/index.html` | recorded/CSS demo inside Mudra's motion grammar, reduced-motion honoured; GitHub/Docs reachable <768px |

---

## 5. Verification matrix

| Claim | Proof | Who |
|---|---|---|
| CA no longer carries a bogus SAN | `tests/cert_strict.rs` assertions + `openssl x509 -noout -text` on a generated CA | CI |
| Apple's stack accepts the chain | macOS `/usr/bin/curl --cacert` (SecureTransport) → 200, in `tests/e2e_securetransport.rs` | CI (macOS) |
| The whole daemon path is fixed | same curl through `antra proxy start` + `antra alias` | CI (macOS) |
| Old installs migrate without a manual dance | `tests/cert_store.rs` rotation tests + a manual temp-`HOME` walkthrough | CI + human |
| Nothing rotates the developer's real CA during `cargo test` | shasum + `security find-certificate` before/after (Phase 0 gate) | human, once per machine |
| Safari actually loads `https://app.localhost` warning-free | manual pass; `antra hosts sync` first | **human only — no GUI here** |
| Firefox loads it | manual pass | **human only** |
| Leaf validity is inside Apple's limit | `check_leaf` assertion (≤825 days) | CI |
| Suite/lint/format | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` (AGENT.md:53-55) | CI |

## 6. Rollout

1. Land Phases 0–2 as one PR (harness + strict gate + generation) — the gate goes red→green inside it, no migration code yet.
2. Land Phases 3–4 as one PR (rotation + doctor + docs), reviewable on its own: nothing rotates until the marker ships.
3. Land Phase 5 as one PR (the e2e gate).
4. Tag `v0.5.0` with the release notes in §4 Phase 4; update `Formula/` and `install.sh` pins.
5. After release, one real-machine pass: legacy install → upgrade → observe the re-trust prompt → Safari + Chrome + Firefox checklist rows.

Rollback: the marker file is the switch. Deleting `~/.config/antra/.ca-version` forces one more rotation; `antra clean` is the full reset. No code path can be left half-migrated, because rotation is idempotent on the marker and the CA write is atomic.

## 7. Decisions taken

1. **Bound the validity in this release, or SAN-only now?** — **Decided: bound it.** mkcert, the tool this project measures itself against, exists precisely because the ≤825-day window is what makes local HTTPS work in Safari; shipping the SAN fix alone would claim Safari support that a 4096-dated leaf can still break.
2. **CA expiry: 2y3m (mkcert parity) or a long-lived CA with a bounded leaf?** — **Decided: bounded, 800 days, CA and leaf alike.** A flat 800 cannot drift over the cap in a leap year the way "2 years 3 months" can, and when the CA does expire the rotation path is already consented, tested and byte-exact.
3. **Renewal threshold** — **Decided: 45 days, silent, logged.** Regeneration falls back to the existing (still valid) certificate if re-signing fails.
4. **Who runs the Safari/Firefox manual pass?** — still a human. The Safari-critical half is now machine-checked by `tests/e2e_securetransport.rs` against Apple's own TLS stack.

## 8. What I did not do during planning

- No product code, docs, or test files changed while writing the plan. Two throwaway probe files and `/tmp/antra-probe` were removed; three local `openssl s_server` probes were stopped; the real config dir and keychain were verified byte-identical afterwards.
- Nothing about the real trust store, `/etc/hosts`, or `antra clean` was touched.
- The test names in §4 were proposals at that point; the ones that shipped are listed in §9.

## 9. Implementation log

| Phase | Landed | Notes |
|---|---|---|
| 0 | `tests/common/mod.rs` (new); both e2e suites routed through `TestHome` | Two things the plan did not anticipate: the shared home must sit under a *short* root, because macOS has no XDG runtime dir (the socket falls back to `$HOME/Library/Application Support/antra/daemon.sock` and blew the 104-byte `sun_path` limit from a deep temp root — now asserted by a test), and the suite must stop its daemon on exit, or the leftover process holds 8443 and the developer's next `antra run` cannot bind. |
| 1 | `Cargo.toml` (`x509-parser`, `time`), `src/certs/validate.rs`, `tests/cert_strict.rs` | Recorded before-state, all red: `CA certificate must carry no subjectAltName, found: DNS:Antra Local CA`; `CA must expire within the 825-day limit, got 755783 days`; `leaf certificate is valid for 774680 days, over the 825-day limit…` |
| 2 | `certs/ca.rs` (`CertificateParams::default()`), `certs/leaf.rs`, `certs/mod.rs` (800-day window), `certs/store.rs` (`LEAF_VERSION` → `"3"`), `certs/cache.rs` (renewal) | Bounded validity for CA *and* leaf, with a comment at the generator explaining why the SAN must not come back. |
| 3 | `certs/store.rs` (`CA_VERSION`, `retired-ca.pem`, `clear_pending_retired_ca`), `trust.rs` (`finish_ca_rotation` on every install path, `retired_ca_pending`), `cli/run.rs` (trust state checked before the "asked before" flag), `ipc/protocol.rs` (`ca_fingerprint`, `PROTOCOL_VERSION` 3), `ipc/server.rs` + `daemon/server.rs` (publish it), `cli/doctor.rs` (strict-CA check, retired-CA warning, daemon-mismatch warning), six new tests in `tests/cert_store.rs` | `PROTOCOL_VERSION` 2 → 3 means a daemon started by the previous binary no longer answers the new CLI — the same truth the doctor warning reports, and the restart a rotation needs anyway. |
| 4 | `README.md:174`, `ROADMAP.md` (#10/#11/#18/#23/#24/C1–C3 → DONE, rows 30–33, legend gained `DONE`), `docs/mvp.md` (Safari row), `docs/security.md` (CA Versioning and Rotation), `docs/architecture.md` (certs tree, new deps), `Cargo.toml` 0.5.0, `install.sh` version examples | `Formula/antra.rb` deliberately untouched: its sha256 values only exist once the release artifacts do. |
| 5 | `tests/e2e_securetransport.rs` | In-process TLS server plus `/usr/bin/curl --cacert` (SecureTransport) → 200 through a real `antra proxy start` + `antra alias`; a sensitivity test re-mints the pre-0.5 CA shape and asserts it is *still* rejected, so the gate cannot pass vacuously. |

### Release notes (v0.5.0) — ready to paste

> **Your local CA is regenerated once.** v0.4.0's root certificate carried a `subjectAltName` of `DNS:Antra Local CA`. A `dNSName` must be a valid DNS name, so Apple's TLS stack — Safari, and every macOS system tool built on it — refused the chain with `SSL certificate problem: unsupported or invalid name syntax` even when the CA was installed and trusted. OpenSSL-based checks accepted it, which is why this went unnoticed.
>
> On the first run after upgrading, Antra replaces the root, asks you to re-trust it (nothing is removed without your consent), then removes the superseded certificate from the trust stores — byte-exactly, so an unrelated "Antra Local CA" you installed by hand is never touched. If your setup pins `NODE_EXTRA_CA_CERTS` or exports `ca.pem` elsewhere, re-read it: the path is unchanged, the contents are not. A daemon started before the upgrade is holding the old root and must be restarted (`antra proxy stop && antra proxy start`); `antra doctor` says so.
>
> Certificate validity is now bounded at 800 days, inside Apple's 825-day ceiling for TLS server certificates. Leafs renew themselves inside a 45-day window, so there is nothing to do.
>
> Also in this release: the test suite runs against a disposable HOME instead of yours; `antra doctor` checks the CA against strict X.509 rules; the ROADMAP and README now match the code.

### Closed since this list was written

- ✅ PRs [#12](https://github.com/ifelse-codes/antra/pull/12) and [#13](https://github.com/ifelse-codes/antra/pull/13) merged; `v0.5.0` tagged and published; `Formula/antra.rb` updated from that release's own `.sha256` assets; the `landing/` site redeployed (deployment `389916d4`) so the served installer matches the repo copy.
- ✅ The Windows CI job now runs `cert_strict` instead of only compiling it.

### Still open

- The manual Safari + Firefox pass on `docs/mvp.md` — no GUI here. The Safari-critical half is machine-checked by `tests/e2e_securetransport.rs` in macOS CI; the browser pass is still a human step.
- Phase 6, unchanged and unstarted: `antra logs` (ROADMAP #32, `NOW`), the `sudo antra alias` hint, underscore domains, landing headers/404, wiring the shell e2e suites into CI, and the shared upstream client (#33).
