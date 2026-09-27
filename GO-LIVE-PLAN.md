# Antra Go-To-Market Plan

---

## ▶ RESUME HERE — next session starts from this block (last updated 2026-09-27)

**Current state: `v0.6.0` is PUBLISHED and the site is live with the matching installer copy.** The working tree is clean; everything shipped through `main`. `v0.5.0` fixed the defect that made HTTPS unusable on Apple's TLS stack (the root CA carried an invalid `subjectAltName`); `v0.6.0` added `antra logs`, pooled upstream connections, two resolver fixes, and the landing security headers + 404.

**DONE & verified:**
- Releases ✅ `v0.5.0` (CA v2) and `v0.6.0` (`antra logs`, pooled upstream client, correctness fixes) both published as Latest, 5 targets each, checksums verified by downloading the artifacts, Homebrew Formula updated from each release's own `.sha256` assets.
- Code ✅ PRs [#12](https://github.com/ifelse-codes/antra/pull/12)–[#17](https://github.com/ifelse-codes/antra/pull/17) merged; all 7 CI jobs green on each. #16 fixed a Linux-only path bug in the new logs tests (caught by the Ubuntu job); #17 de-flaked a pre-existing port race in a websocket test.
- Gates ✅ `tests/cert_strict.rs` (strict X.509 rules) and `tests/e2e_securetransport.rs` (macOS: `/usr/bin/curl --cacert` through a live daemon) are the regression gates for the CA work. Rationale and evidence: `fix-plan-2026-09-26-ca-trust.md`, `deep-dive-report-2026-09-26.md` §F.
- Sites ✅ `antra.iifelse.com` deployed for v0.6.0 (deployment `a6908b4f`): `/install.sh` → `Content-Type: text/plain` serving the `v0.6.0` pin; security headers present; unknown paths return 404. Note for whoever deploys next: run `wrangler pages deploy .` from `main` (or pass `--branch main`), or it creates a *branch* deployment and production silently keeps the old copy.

**LATEST VERIFICATION — 2026-09-27 (v0.5.0):**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the full suite green in a disposable HOME (216 executions, 0 failures); all 7 CI jobs green on both PRs.
- `/usr/bin/curl --cacert` (Apple's SecureTransport — the stack that rejected the old CA) returns 200 through a live `antra proxy start` + `antra alias`; an untrusted chain is still refused. Now runs in macOS CI as `tests/e2e_securetransport.rs`.
- Migration rehearsed in a disposable HOME: a ≤0.4.0 install rotates once, prompts to re-trust, and has the superseded root removed byte-exactly; `doctor` reports the strict-CA result and a pending retired CA.
- Safari and Firefox were **not** opened in a browser — the MVP checklist in `docs/mvp.md` leaves those boxes unchecked by design.

**VERIFICATION — 2026-09-25 (v0.4.0 era, historical):**
- `cargo fmt`, `cargo check --all-targets`, Clippy, release build, doc tests, and **299 test executions** passed.
- Chrome 153 + Vite 8.2.1 passed through the proxy: HTTP 200, proxy-origin `wss://` HMR socket, Vite `update` frame, no direct backend WebSocket, and clean Ctrl+C route/process cleanup.
- A real PTY verified both CA consent answers: `n` skips trust; Enter installs the disposable CA. The disposable CA was removed afterward and the pre-existing Keychain certificate stayed unchanged.
- `run`, `add route`, `alias`, and `proxy start --route` reject unapproved custom domains before daemon or hosts mutation.
- Firefox was not run. The full `antra clean` command was not run against the real `/etc/hosts` because it contains active Antra entries; the managed-block transformation is hermetically tested.

**DO NOT REDO / SAFETY NOTES:**
- Run Rust tests with a disposable `HOME`; preserve `CARGO_HOME=/Users/suman/.cargo` and `RUSTUP_HOME=/Users/suman/.rustup` so rustup still works.
- Do not run `antra clean` against the real hosts file; it contains active Antra-managed entries.
- Chrome transport/HMR used a disposable untrusted CA with `ignoreHTTPSErrors`; no-warning certificate proof still requires explicit real-CA trust approval.
- Windows hermetic CI is configured, but local cross-compilation is blocked by missing `x86_64-w64-mingw32-gcc`; use GitHub Actions for Windows runtime verification.
- No commit, tag, release, or deployment was made for the hardening changes.

**NEXT SESSION — remaining open work (all need a human; none block the published release):**
1. **Plausible analytics** — create account, register domain `antra.iifelse.com` (tag is live but no account → no data collects). This is the single highest-value item.
2. **Phase 2 launch comms** — publish social posts (draft Tweet is in the "Asset Templates" section below): Tweet, Hacker News, Product Hunt, Lobsters; email dev newsletters.
3. **GitHub Discussions** — enable on `ifelse-codes/antra`.
4. **Social proof** — testimonials, "Used by" section, GitHub Stars count on landing.
5. **DX assets** — 60-sec quick-start video, examples repo (Vite/Next/Express), CLI reference page at `antra.iifelse.com/cli`.
6. **Phase 3** — Discord/Telegram community channel; roadmap NOW items.
7. **Security release gate** — run the new Windows CI job, complete the formal browser checklist after explicit CA trust approval, then review and release the hardening changes.

**Reusable gotchas to remember (they cost time this session):**
- Cloudflare Pages **Function is silently shadowed by a static file at the same path** — use `landing/_headers` instead of a Function.
- GitHub API **rejects `--latest` on a draft release** — edit notes/title first, then `--draft=false --latest` in a second call.
- Custom-domain curl shows **stale edge cache** — verify with a cache-busting `?<timestamp>` query param after deploy.

---

## Project Readiness Assessment

### Product-Market Fit: STRONG ✅

Antra solves a real, painful problem: developers hate port-based URLs (`localhost:5173`), browser security warnings, and complex local dev setups. The product is **functionally complete** and ready for launch.

### Engineering Status: v0.6.0 PUBLISHED

- **Published version:** 0.6.0 (was 0.5.0)
- **Local verification:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the full suite passed in a disposable HOME
- **CI/CD:** macOS + Ubuntu full tests, Windows hermetic tests (now running `cert_strict`), and a macOS SecureTransport certificate test — all green on PRs #12–#17
- **Release workflow:** Automated cross-platform builds with checksums; artifacts downloaded and verified
- **Certificate correctness:** root CA no longer carries a SAN, validity bounded to 800 days, existing installs rotate once with byte-exact removal of the superseded root (`docs/security.md`)
- **Documentation:** security behaviour, rotation policy, and acceptance evidence recorded below and in `fix-plan-2026-09-26-ca-trust.md`

### Gaps Before Release ⚠️

| Category | Status | Priority |
|----------|--------|----------|
| Security hardening | ✅ Shipped in v0.5.0 and v0.6.0 | — |
| Browser certificate acceptance | ✅ Machine-checked on Apple's TLS stack (`tests/e2e_securetransport.rs`); Safari + Firefox browser pass still open | Human pass on `docs/mvp.md` |
| Windows CI execution | ✅ Green on PRs #12–#17 | — |
| Landing redeploy | ✅ Done — v0.6.0 assets deployed (`a6908b4f`) | — |
| Shell e2e suites in CI | ⏳ Harness bug fixed (they had never run); wiring blocked on the auto-started daemon having no port override | ROADMAP #21, then wire them |
| Analytics / launch work | ⏳ Later scope | Follow Phase 2/3 plan |

---

## Go-Live Plan (GTM Engineer Recommendations)

### Phase 1: Pre-Launch (Week 1)

#### Critical Fixes
1. **Fix install URL in README**
   - README has `curl -fsSL https://raw.githubusercontent.com/.../install.sh`
   - Should point to `https://antra.iifelse.com/install.sh`
   - Ensure Cloudflare Worker routes `/install.sh` with `Content-Type: text/plain`

2. **Domain setup**
   - Purchase `antra.dev` (ideal: short, memorable, professional)
   - Redirect `antra.io`, `antra.local` if budget allows
   - Update all docs to use canonical domain

#### Legal & Trust
3. **Privacy Policy** - Add to website (required for macOS trust store install)
4. **Terms of Service** - Basic TOS page
5. **Security disclosure** - Add `SECURITY.md` with responsible disclosure policy

---

### Phase 2: Launch (Week 2)

#### Marketing Assets
6. **Launch Checklist:**
   - [ ] GitHub release `v0.4.0` (minor bump for public launch)
   - [ ] Tweet from personal + project accounts
   - [ ] Submit to Hacker News, Product Hunt, Lobsters
   - [ ] Email dev newsletter (LD News, Product Hunt newsletter)
   - [ ] GitHub Discussions enabled

7. **Social Proof:**
   - Add "Testimonials" section to website
   - Include screenshots from early users (if any)
   - Add "Used by" section if any known projects use it

#### Developer Experience
8. **Quick Start Video** - 60-second Loom recording
9. **Examples repo** - GitHub repo with demo projects (Vite, Next.js, Express)
10. **CLI reference** - Document all commands at `https://antra.iifelse.com/cli`

---

### Phase 3: Post-Launch (Week 3-4)

#### Metrics & Feedback
11. **Setup analytics:**
    - Plausible (privacy-friendly) or Posthog (free tier)
    - Track: install count, most used commands, errors

12. **GitHub Issues template:**
    - Bug report, feature request, question templates

13. **Create discord/telegram** - Community support channel

#### Feature Momentum
14. **Ship from roadmap NOW items:**
    - OS service install (critical for production-like HTTPS)
    - Custom TLD support (needed for OAuth redirects)

---

## Competitive Positioning

| Feature | Antra | portless | ngrok | Cloudflare Tunnel |
|---------|-------|----------|-------|-------------------|
| Local only | ✅ | ✅ | ❌ | ❌ |
| No account | ✅ | ✅ | ❌ | ❌ |
| One command | ✅ | ⚠️ More complex | ✅ | ❌ |
| HTTPS | ✅ | ✅ | ✅ | ✅ |
| WebSocket/HMR | ✅ | ✅ | ✅ | ⚠️ Config |
| Open source | ✅ | ✅ | ❌ | ❌ |

**Positioning:** "The open-source alternative to ngrok for local development"

---

## Final Verdict

### Ready to Launch? YES ✅

**What's working:**
- Core functionality is complete and tested
- Cross-platform support (macOS, Linux, Windows)
- Professional website with landing page
- GitHub Actions CI/CD and release workflow
- Comprehensive documentation

**Before public launch:**
1. Fix install URL in README
2. Purchase official domain (`antra.dev` recommended)
3. Add privacy policy
4. Create launch announcement content

**After launch:**
- Focus on community building
- Ship OAuth support (custom TLD) - critical for modern dev workflows
- Add usage analytics

**Recommended launch date:** Next Monday (2026-09-28) - gives you a week for fixes and announcement prep.

---

## Quick Action Items (Do Now)

### Immediate Fixes
```bash
# Check install.sh is accessible
curl -s https://antra.iifelse.com/install.sh | head -5

# Verify binary builds
cargo build --release
./target/release/antra --version
```

### Launch Tasks
1. Purchase `antra.dev` domain ($10-15/year)
2. Add privacy policy to website
3. Create GitHub issue templates
4. Draft launch announcement

---

## Session Progress — 2026-09-22

Work completed this session against the plan above. Canonical domain decision:
**`antra.iifelse.com` only** (no marketing domain purchased; the plan's `antra.dev` item is dropped).

### Phase 1: Pre-Launch — DONE

| Plan item | Status | What was done |
|-----------|--------|---------------|
| 1. Fix install URL in README | ✅ Done | Both curl one-liners in `README.md` now point to `https://antra.iifelse.com/install.sh` |
| 2. Route `/install.sh` (Worker) | ✅ Done (needs deploy) | Added `landing/functions/install.sh.ts` — Cloudflare Pages Function serving `/install.sh` with `Content-Type: text/plain` |
| 3. Domain setup / canonical domain | ✅ Done | No new domain (using `antra.iifelse.com`). Removed `raw.githubusercontent` fallback from root + `landing/install.sh` usage comments; all docs already canonical |
| 4. Privacy Policy | ✅ Done | New `landing/privacy.html`, linked in footer + nav |
| 5. Terms of Service | ✅ Done | New `landing/terms.html`, linked in footer + nav |
| 6. Security disclosure | ✅ Done | New root `SECURITY.md` (responsible disclosure, security model, scope) |

### Phase 2: Launch — PARTIAL

| Plan item | Status | Note |
|-----------|--------|------|
| 1. GitHub release `v0.4.0` | ✅ **DONE 2026-09-23** | Tag pushed, built, verified, **published** as latest. See Session Progress below |
| 2. Social posts (Tweet/HN/PH/Lobsters) | ⏳ Pending | Out of session scope |
| 3. GitHub Discussions | ⏳ Pending | Out of session scope |
| 4. Testimonials / "Used by" | ⏳ Pending | Needs real users |
| 5. Quick Start Video | ⏳ Pending | Out of session scope |
| 6. Examples repo | ⏳ Pending | Out of session scope |
| 7. CLI reference page | ⏳ Pending | Out of session scope |

### Phase 3: Post-Launch — PARTIAL

| Plan item | Status | Note |
|-----------|--------|------|
| 11. Setup analytics | ✅ Done | Plausible snippet added to `landing/index.html` (`data-domain="antra.iifelse.com"`); needs account registration + deploy |
| 12. GitHub Issues templates | ✅ Done | Created `.github/ISSUE_TEMPLATE/`: `bug_report.yml`, `feature_request.yml`, `question.yml` |
| 13. Discord/Telegram community | ⏳ Pending | Out of session scope |
| 14. Ship OS service install | ✅ Already shipped | `antra service install\|status\|uninstall` (launchd/systemd/sc.exe) verified present in CLI |
| 14. Custom TLD support | ✅ Already shipped | `antra run --tld <TLD>` + `--allow-custom-domain` verified present in CLI (auto-hosts-sync included) |

### Verification

- **Install script reachable** ✅ `https://antra.iifelse.com/install.sh` → HTTP 200, real shell script. Live `Content-Type` is `application/x-sh`; will become `text/plain` once the Pages Function is deployed. Live script pins `v0.3.0` (site is one deploy behind `v0.3.1`).
- **Release build** ✅ `cargo build --release` clean → `antra 0.3.1`. Full test suite passes, 0 failures.

### Outstanding deployment steps (not part of repo changes)

1. ~~Redeploy landing site~~ ✅ **DONE 2026-09-23** — `wrangler pages deploy . --project-name antra-landing`. Publishes `install.sh` (v0.4.0), privacy/terms pages, analytics tag.
2. ~~Register `antra.iifelse.com` in Plausible~~ ⏳ **Still requires account-side registration** — snippet is live (`data-domain="antra.iifelse.com"`), but no Plausible account/domain registered yet, so no data collects.
3. ~~After deploy, re-verify `/install.sh` returns `Content-Type: text/plain`~~ ✅ **DONE 2026-09-23** — verified `text/plain`.
4. ~~Re-deploy the `v0.4.0` release~~ ✅ **DONE 2026-09-23** — tag `v0.4.0` pushed, release workflow running; artifacts build cross-platform.

---

## Session Progress — 2026-09-23 (Go-Live / v0.4.0)

### Site published ✅

- Landing site redeployed to Cloudflare Pages (project `antra-landing`) on 2026-09-23.
- `/install.sh` now returns `Content-Type: text/plain` **verified** on the custom domain.
- `landing` now ships: `index.html` (analytics tag, v0.4.0 pin), `install.sh` (v0.4.0), `privacy.html`, `terms.html`, `_headers`.
- **Fix applied:** the planned `landing/functions/install.sh.ts` Pages Function was **shadowed by the static `install.sh`** (Cloudflare Pages serves the static file for an exact-matching path, function never ran). Replaced with a `landing/_headers` file that pins `Content-Type: text/plain` on the static asset directly; removed the dead function.

### v0.4.0 release PUBLISHED ✅

- Version bumped `0.3.1 → 0.4.0` across `Cargo.toml`, `Cargo.lock`, `README.md`, `install.sh`, `landing/install.sh`, `landing/index.html`, issue-template placeholders.
- `cargo build --release` clean → `antra 0.4.0`; full test suite passes (61 unit/integration + doc tests).
- Tag `v0.4.0` pushed → `Release` workflow built 5 targets (macOS x2, Linux x2, Windows) with checksums.
- **Artifacts + checksums verified** — downloaded all 5 binaries, `shasum -a 256` matches every published `.sha256`:
  - `antra-aarch64-apple-darwin` `b7e47450…`
  - `antra-x86_64-apple-darwin` `4a201bcf…`
  - `antra-aarch64-linux` `4c5072d9…`
  - `antra-x86_64-linux` `5c7bdfba…`
  - `antra-x86_64-windows.exe` `05a59c40…`
- **Homebrew Formula updated** to `v0.4.0` with the new per-target sha256 (committed + pushed).
- **Published 2026-09-23 15:54 UTC** as `isDraft: false`, `prerelease: false`, marked **Latest**.
  - Title: "Antra 0.4.0 — public launch" with full release notes (replaced the thin auto-generated body).
  - URL: https://github.com/ifelse-codes/antra/releases/tag/v0.4.0
  - 10 assets attached (5 binaries + 5 `.sha256`).
  - **Note:** setting `--latest` on a draft is rejected by the GitHub API (`Latest release cannot be draft or prerelease`) — must edit notes/title first while still draft, then set `--draft=false --latest` in a second call, then re-verify.

### Session close-out — 2026-09-23 (Go-Live / v0.4.0)

**Publishing work DONE this session (all committed to `origin/main`):**

| Commit | What |
|--------|------|
| `965bbab` | `release: bump to v0.4.0` — version bump across Cargo, README, install.sh, landing, issue templates |
| `c2386f2` | `docs: pre-launch site content...` — privacy/terms, SECURITY.md, ISSUE_TEMPLATEs, GO-LIVE-PLAN.md, Pages function |
| `0c6e38d` | `fix: set install.sh Content-Type text/plain via _headers` — **killed the shadowed Pages Function** |
| `94f97c3` | `docs: record go-live site deploy & release in plan` |
| `fb4a45a` | `chore: homebrew formula tracks v0.4.0 with release sha256` |
| `7c8b5fa` | `docs: record v0.4.0 artifacts verification in go-live plan` |

**Key finding worth remembering:** a Cloudflare Pages **Function does NOT run when a static file shares the exact same path** — the static asset wins and the Function is silently shadowed (no error). Use a `landing/_headers` file to set headers on static assets instead; it's simpler and reliable. The original `landing/functions/install.sh.ts` approach was dead code and was removed.

**Verification evidence:**
- `https://antra.iifelse.com/install.sh` → HTTP 200, `Content-Type: text/plain`, v0.4.0 content (verified with cache-busting `?<timestamp>` query param — the custom domain initially returned stale edge-cache `application/x-sh` until cache-bypass confirmed the new deploy).
- `https://antra.iifelse.com/privacy.html` / `terms.html` → 200.
- `antra 0.4.0` build + full test suite green.
- Release `v0.4.0` live as latest with verified checksums.

**Remaining after close-out (unchanged scope — decisions/actions outside release):**
- **Plausible**: account does not exist; the tag is live but no account/domain registered → analytics won't collect until then.
- Phase 2 launch comms: social posts (Tweet/HN/PH/Lobsters), GitHub Discussions, testimonials/"Used by", quick-start video, examples repo, CLI reference page.
- Phase 3: community channel.

---

## Session Progress — 2026-09-25 (Security hardening and acceptance)

### Completed in the working tree

- **CA consent:** `antra run` and `antra dev` now ask `Install the Antra local CA? [Y/n]` on an interactive first run. Enter means yes; `--yes` is an explicit non-interactive opt-in; `--no-trust-prompt` skips the flow. Non-interactive sessions never install trust without `--yes`.
- **Custom-domain safety:** `.localhost`, `.test`, `.local`, and `.internal` remain automatic. Every other hostname requires `--allow-custom-domain` across `run`, `dev`, `alias`, `add route`, and `proxy start --route`. Approved public-looking TLDs produce a warning.
- **Reversible cleanup:** `antra clean` now removes the exact current CA from applicable system, macOS login-keychain, and Windows CurrentUser trust stores, removes the complete Antra-managed hosts block, and only then deletes local state. Missing trust/hosts entries are idempotent; malformed state aborts safely.
- **Process isolation:** Unix child processes and the auto-started daemon now get separate process groups, so Ctrl+C cleanup does not target the daemon accidentally.
- **Windows CI:** Added a Windows hermetic test job. It compiles every test target, executes the safe library/integration allowlist serially, and excludes the known process-tree and real-state E2E targets.

### Verification completed

- `cargo fmt --all`
- `cargo check --all-targets`
- `cargo clippy --all-targets -- -D warnings`
- Full test suite with a disposable `HOME`: **299 test executions passed, 0 failed**
- Real Chrome 153 + Vite 8.2.1: page loaded through Antra, HMR connected via `wss://antra-v04.localhost:19443`, a Vite `update` frame arrived, and no direct backend WebSocket was used.
- Ctrl+C removed the test route and Vite process cleanly.
- Interactive CA consent test passed: a real PTY showed `Install the Antra local CA? [Y/n]`, accepted `n`, skipped trust installation, and cleaned the route.
- Default-Enter consent test passed: pressing Enter installed the disposable CA, and the follow-up removal left the pre-existing keychain certificate unchanged.
- CLI policy check passed: `run`, `add route`, `alias`, and `proxy start --route` all rejected an unapproved custom domain before daemon/hosts mutation; the explicit approval path was accepted.
- Disposable macOS trust test passed: a temporary CA was installed into the login keychain, removed with the exact-removal path, and the pre-existing Antra-named certificate remained unchanged.
- No-warning certificate acceptance was **not claimed**: the user chose Chrome-only testing without changing the Keychain, so the disposable test CA was intentionally untrusted. The browser correctly returned `ERR_CERT_AUTHORITY_INVALID` with certificate validation enabled.
- Firefox was not run because it is not installed and was intentionally left out of this pass.

### Still open

- Run the formal MVP checklist in Chrome and Firefox after the user explicitly authorizes refreshing the current Antra CA trust.
- The full `antra clean` orchestration was not run against the real `/etc/hosts` because it contains active Antra entries; its managed-block transformation is covered by hermetic tests.
- Run the new Windows CI job on GitHub; the hermetic command is configured, but local Windows cross-compilation is blocked by the missing `x86_64-w64-mingw32-gcc` toolchain.
- Keep the later launch work unchanged: Plausible registration, Discussions, launch communications, social proof, video/examples/CLI reference, community channels, LICENSE, and security-contact work.
- This hardening is currently an uncommitted working-tree change; no release or deployment was made.

---

## Asset Templates

### Launch Tweet
```
🚀 Antra 0.4.0 is live!

Stable HTTPS domains for local development.
One command. Real URLs. No ports.

antra run --domain myapp.localhost -- pnpm dev
→ https://myapp.localhost

Built in Rust. No cloud. No accounts. No telemetry.

👉 https://antra.iifelse.com
 GitHub: https://github.com/ifelse-codes/antra
```

### Landing Page Sections to Add
- Testimonials
- Privacy Policy link in footer
- GitHub Stars count
- "Used by" logos (if any)

---

*Generated by GTM Engineer on 2026-09-21*
