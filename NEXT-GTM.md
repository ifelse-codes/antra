# NEXT-GTM.md — Prompt for the next session

> Feed this file (`@NEXT-GTM.md`) to the agent at the start of the next session.
> It is self-contained: it tells the agent what to read, what to ask, and which
> lanes to work. Everything it needs to know is here or in the files it names.
>
> **Also load the skill in that session:** invoke `/skill:darshan-npt` (or paste
> `SKILL.md` from `~/.omp/agent/skills/darshan-npt/`) like the user did this
> session — the agent should read the skill first and follow it for every reply.

## ✅ STATUS 2026-10-04 — all three lanes drafted

Read this first, then the job below it.

`launch/` now holds the three deliverables, drafted and fact-checked:

- **`launch/launch-post.md`** — full first draft, headline options marked for
  voice edits. Facts traced to README / landing / the published binary.
- **`launch/comparison.md`** — ngrok, mkcert, portless, Caddy, Tailscale
  Funnel, every number sourced and dated. Includes a proposed replacement
  table.
- **`launch/mac-checks-runbook.md`** — **Check 2 closed and verified**;
  Check 1 is a ~1-minute block whose baseline is already measured.

**Two findings the next session must not rediscover:**

1. **The direct competitor is portless, not ngrok.** `vercel-labs/portless`
   was created **2026-02-15** — *after* `docs/research/portless.md` first
   noted it — is now a **Vercel Labs** product with **12,642 stars / 431 forks**,
   pushed 2026-10-03, Apache-2.0, and is actively developed. It does the same
   job as Antra and beats it on HTTP/2, framework-aware flag injection, LAN
   mode, monorepo workspaces, worktree subdomains, wildcard routing and
   adoption. Antra's honest edge: language-agnostic (no Node 24 required), one
   native binary, no sudo for trust on macOS, MIT. **Do not write a
   comparison that omits portless or claims Antra invented the idea.**
2. **No `LICENSE` file exists.** `Cargo.toml` declares MIT, the README shows an
   MIT badge, and the GitHub API reports `"license": null`. One-file fix, not
   yet made — it is a legal-branding gap that matters the moment a stranger
   looks, so it needs a maintainer decision on the copyright line.

**Check 2 (Firefox on a Mac) is DONE and green — both legs.** The automated leg
ran against the real `HOME` so the CA was the trusted one: `8 pass / 0 fail /
0 expected-fail / 1 skip`, Chrome + Firefox + WebKit all `200` with no
certificate error. Then Firefox **157.0** was installed and the real
application loaded an Antra URL with no warning, confirmed by screenshot rather
than by eye. **C19 is Linux-only. This Mac is done; nothing is owed here.**

## One job

**Get Antra to market.** The product is done (v0.6.5, published 2026-10-04);
this session is GTM, not code. Draft the three launch materials below and let
the user pick them apart in short rounds. Do the bulk; hand only voice-level
review to the user.

## Project state (so you don't re-derive it)

- **Antra**: native Rust CLI that fronts a local dev server with a stable HTTPS
  domain. One command: `antra run --domain myapp.localhost -- pnpm dev` →
  `https://myapp.localhost`. No ports, no `/etc/hosts`, no cert warnings.
- **v0.6.5 published**, all 10 phases done, release-check green on 5 legs
  (Homebrew + `curl | bash` on macOS/Linux + live site).
- **C1–C26 all done** — includes the CA-rotation fix (C23, worst bug), installer
  (C18/C26), first-run fixes (C24–C26).
- **Tests**: ~500 Rust passing; 4 shell e2e suites 206 passing / 0 failing /
  9 skipped, all in CI.
- git `main` clean; only stray untracked `tests/fixtures/`.
- Read `AGENT.md` ("do not redo blindly" section), `docs/security.md`,
  `ROADMAP.md`, `README.md` before drafting — they carry the facts and the
  voice.

## The three lanes (user will pick one or more)

1. **Launch post draft** — headline → problem → one command → why better →
   install → close. Write the full first draft. User edits the voice.
2. **Comparison** vs ngrok / mkcert / portless — price, setup, limits, when to
   use each. Web-research current numbers; the finished doc feeds the launch
   post + landing page.
3. **Mac checks runbook** — exact 2-step checklist: one `curl | bash` from a
   fresh `HOME` on a real Mac (A2's stall was there, never explained), and
   opening Firefox once on a Mac. **The Firefox half is now closed and green;
   only the installer half is outstanding.**

All three are drafted as of 2026-10-04. What remains is **voice-level review by
the user**, plus the two open items in the STATUS block at the top: the
LICENSE file, and Check 1's manual run.

## How to interact this session

1. Start: summarize state in one glance (Darshan), then ask which lane to open.
2. Work the chosen lane in short rounds — produce, user tweaks, produce again.
3. Every reply: follow Darshan (glance first, structure, plain words at the
   end). User picks by replying; do not stall to collect info already given.

## Optional lanes (only after the big three; ask the user)

- Draft the brand positioning: "local-first tunnel" vs "portless alternative".
- Refresh landing-copy angle for the launch.

## Do NOT

- Touch real code — no new features unless a launch report surfaces a bug.
- Re-open the Linux Chrome/Firefox cert gap (C19) — decided, stays closed.
- Re-add the manual Safari/Firefox pass to "still owed".
- Publish anything — a person presses go.

## Deliverable

A `launch/` folder in the repo (or clearly named files) containing the drafted
launch post, comparison, and runbook, whatever the user picked. Plus this file
updated with what was done, so the NEXT-next session starts clean.

Work until the user says stop; don't narrow scope without asking.
