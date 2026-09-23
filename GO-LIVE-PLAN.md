# Antra Go-To-Market Plan

## Project Readiness Assessment

### Product-Market Fit: STRONG ✅

Antra solves a real, painful problem: developers hate port-based URLs (`localhost:5173`), browser security warnings, and complex local dev setups. The product is **functionally complete** and ready for launch.

### Engineering Status: PRODUCTION-READY ✅

- **Version:** 0.4.0
- **All 10 phases complete** (CLI, proxy, HTTPS, WebSocket, CA trust, daemon/IPC, cross-platform)
- **Tests:** 62/62 passing
- **CI/CD:** GitHub Actions for macOS, Linux, Windows with clippy, fmt, tests
- **Release workflow:** Automated cross-platform builds with checksums
- **Build:** Zero warnings (`cargo clippy -- -D warnings` passes)
- **Documentation:** Comprehensive README, architecture docs, security docs, roadmap

### Gaps Before Launch ⚠️

| Category | Status | Priority |
|----------|--------|----------|
| Website | ✅ Built | Deploy to `antra.iifelse.com` |
| Install script | ⚠️ README has wrong URL | Fix `install.sh` routing |
| Install.sh | ✅ Landing page has working script | Use `https://antra.iifelse.com/install.sh` |
| Domains | ⚠️ No marketing domain purchased | Get `antra.dev` or similar |
| Analytics | ❌ None | Add lightweight analytics (e.g., Plausible) |
| Pricing page | ❌ Missing | Consider free tier + pro features |

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
