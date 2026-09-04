# Antra — User Experience Test Prompt

> Reusable prompt for testing Antra as a complete stranger.
> Feed this to an AI assistant (or use yourself) to get an unbiased outside perspective.
> Adjust the project URL and features as needed for future re-tests.

---

## The Prompt

```
You are a software developer who has never heard of Antra before.

You stumbled upon this website: https://antra.iifelse.com

Your task: Go through the FULL journey of discovering, installing, and using this tool as a complete stranger. You are NOT allowed to look at the source code, README, or any internal docs. Only use what a normal user would see: the website, the install script, the CLI --help output, and the tool itself.

## Phase 1: Discovery (Website)

1. Visit https://antra.iifelse.com
2. Read the landing page as if you've never seen it before
3. Answer these questions:
   - What does this tool do? (in your own words)
   - What's the promise? What does it claim to deliver?
   - Does the website make you want to try it? Why or why not?
   - What's confusing or unclear on the page?
   - What questions do you have before installing?

## Phase 2: Installation

1. Follow the install instructions on the website (the curl one-liner)
2. If the install fails or asks unexpected questions, note it
3. Answer:
   - How smooth was the install? Any surprises?
   - Did you feel confident it was safe to run?
   - Any steps that felt unnecessary or confusing?

## Phase 3: First Run

1. Run `antra --help` and read the output
2. Run `antra doctor` to see your setup status
3. Try to follow the "Quick start" from the website
4. Answer:
   - What does `--help` tell you? Is it enough to get started?
   - What does `doctor` show? Do you understand the output?
   - Could you figure out what to do next without external help?

## Phase 4: Core Usage

1. Start a simple local server (e.g., `python3 -m http.server 8080`)
2. Try `antra run --domain test.localhost -- pnpm dev` or similar
3. Try to access the proxied URL in your browser or via curl
4. Try `antra list` to see active routes
5. Try `antra proxy start`, `antra proxy stop`, `antra proxy status`
6. Try `antra alias myapp.localhost 8080` and access it
7. Answer:
   - Did it work on the first try?
   - If it failed, what was the error? Was it helpful?
   - Did you get a working HTTPS URL?
   - How did the output feel — clear, confusing, too verbose, too quiet?

## Phase 5: Error Paths

1. Try to use a port that's already in use
2. Try to access a domain that has no route
3. Try `antra proxy stop` when the daemon isn't running
4. Try `antra clean` and answer the prompt
5. Answer:
   - Were errors helpful or cryptic?
   - Did you know how to fix each problem?
   - Which error message confused you most?

## Phase 6: Comparison

Answer these final questions:

| Question | Your Answer |
|----------|-------------|
| On a scale of 1-10, how easy was this to set up? | |
| On a scale of 1-10, how well does it deliver on its promise? | |
| Would you use this regularly? Why or why not? |
| What's the #1 thing that would make you stop using it? | |
| What's the #1 thing that would make you recommend it? | |
| What surprised you (good or bad)? | |
| What feature is missing that you expected? | |
| How does this compare to localhost:port? | |
| How does this compare to ngrok? | |
| Any final feedback for the developer? | |

## Phase 7: Specific Promises Check

The website makes these specific promises. For each one, test it and say whether it delivers:

| Promise | Delivers? | Evidence |
|---------|-----------|----------|
| "One command" — works with a single CLI invocation | | |
| "Real HTTPS" — no browser warnings | | |
| "No ports" — clean URLs without :5173 | | |
| "No /etc/hosts" — no manual file editing | | |
| "No certificate warnings" | | |
| "WebSocket/HMR support" — Vite, Next.js hot reload works | | |
| "Language-agnostic" — works with any process | | |
| "No cloud, no accounts, no telemetry" | | |
| "Multi-app routing" — multiple domains, one daemon | | |
| "Project config" — antra.toml for team setup | | |
```

---

## How to Use This

1. **Copy the prompt above** into a new conversation with any AI assistant
2. **Or run it yourself** — go through each phase manually and fill in the answers
3. **After each test run**, save the results with a date stamp:
   ```
   tests/user-test-2026-09-03.md
   ```
4. **Compare results** across test runs to track improvement

---

## Customization

To adapt this for a different project, change:

- **Website URL** — replace `https://antra.iifelse.com` with your landing page
- **Install command** — update the curl one-liner or install method
- **Core features** — replace the Phase 4 steps with your tool's main workflow
- **Promises** — update the Phase 7 table with your actual marketing claims
- **Error paths** — add your known edge cases to Phase 5

---

## Suggested Test Cadence

| When | What to test |
|------|-------------|
| Before each release | Full run-through (all 7 phases) |
| After each major fix | Phases 3-6 (skip discovery/install) |
| After landing page change | Phases 1-3 (first impressions) |
| Monthly | Full run with a fresh perspective |
