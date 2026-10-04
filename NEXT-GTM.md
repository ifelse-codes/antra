# NEXT-GTM.md — Prompt for the next session

> Feed this file (`@NEXT-GTM.md`) to the agent at the start of the next session.
> It is self-contained: it tells the agent what to read, what to ask, and which
> lanes to work. Everything it needs to know is here or in the files it names.
>
> **Also load the skill in that session:** invoke `/skill:darshan-npt` (or paste
> `SKILL.md` from `~/.omp/agent/skills/darshan-npt/`) like the user did this
> session — the agent should read the skill first and follow it for every reply.

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
   opening Firefox once on a Mac (never done). Write it so the user just follows
   steps and reports back; you interpret.

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
