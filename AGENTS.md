# AGENTS.md — Mandatory session boot (Antra)

> Auto-loaded every session in this repo. Follow it start to finish; do NOT
> ask "what should I do" — the plan is already collected. Work, then show.

## 1. Boot ritual — always, first turn

1. **Read the Darshan skill** at `skill://darshan-npt` (global user skill,
   `~/.omp/agent/skills/darshan-npt/SKILL.md`; if `skill://` fails, read that
   absolute path). Follow it for **every reply to a human**: glance first,
   structure over prose, never drop a fact.
2. **Read the plan**: `NEXT-GTM.md` (the ready-to-work prompt) and
   `NEXT-SESSION.md` (state + handoff). Also skim `README.md` if needed.
3. **Start executing** — no confirmation round. Ask only for a genuinely
   material decision the user has not already made.

## 2. Current standing work (until a release/ABL says otherwise)

- **GTM, not code.** Product is done and published (v0.6.5). Run the lanes in
  `NEXT-GTM.md`: launch post draft → comparison vs ngrok/mkcert/portless →
  Mac checks runbook. Produce in short rounds, hand voice-level review to user.
- **On user-reported bugs/regressions only:** fix under the Gates in
  `NEXT-SESSION.md` (fmt, clippy -D warnings, full tests, shell suites), keep
  docs/releases/ROADMAP in sync, commit on a feature branch, PR to `main`.

## 3. Do NOT (standing decisions)

- Re-open the Linux Chrome/Firefox cert warning gap (C19) — decided, stays
  closed.
- Re-add the manual Safari/Firefox pass to "still owed".
- Build roadmap features (LAN, monorepo, Tailscale/ngrok, …) unless the user
  explicitly asks after launch.
- Publish a release or go-live artifact — a person presses go.

## 4. Plain words at the end

Every reply's close: one or two everyday sentences — what was done, what it
means for the user, the one thing to do next. No project jargon.
