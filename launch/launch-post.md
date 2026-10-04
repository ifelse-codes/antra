# Launch post — draft v1

> Voice review applied 2026-10-04. Decisions: headline = the command itself;
> title = "Your local app, at a real HTTPS URL"; the comparison — naming the
> direct competitor honestly — ships in the post, the README and the landing
> page; demo = capture script (see `demo-capture.sh` + `demo-shot-list.md`).
> Facts drawn from `README.md`, `landing/index.html` and the published v0.6.6
> binary; nothing here is aspirational. Companion doc:
> [`comparison.md`](comparison.md) — source of truth for the tables.

---

# `antra run --domain myapp.localhost -- pnpm dev` → https://myapp.localhost

**Your local app, at a real HTTPS URL — no ports, no `/etc/hosts`, no warnings.**

---

Local development still looks like 2009.

| You wanted | You got |
|---|---|
| A URL | `localhost:5173` |
| HTTPS | A red lock and a "proceed anyway" click |
| Multiple services | A spreadsheet of ports |
| Cookies, Service Workers, WebCrypto | "This is not a secure context" |
| A teammate to hit the same app | "wait, which port was auth on?" |

The workaround stack is worse than the problem. Edit `/etc/hosts`. Run `mkcert`.
Write a Caddyfile. Remember to trust a CA. Then watch HMR break because the
proxy doesn't tunnel WebSockets.

**One command replaces the pile:**

```bash
antra run --domain myapp.localhost -- pnpm dev
```

```
✓ Domain resolved: myapp.localhost
✓ Proxy ready
✓ HTTPS ready
✓ Route registered

  → https://myapp.localhost
```

Open the URL. Your app is there. Vite HMR still works. Cookies are a secure
context. Nobody typed `:5173`.

The first time, Antra asks one question — `Use port 443? [Y/n]` — because a
URL with no port number needs port 443, and only an admin can open it. You type
your password once; the proxy gives up admin rights as soon as the port is
open. Say no and you get `https://myapp.localhost:8443` instead.

## Install

```bash
# macOS / Linux
curl -fsSL https://antra.iifelse.com/install.sh | bash

# or Homebrew
brew install ifelse-codes/antra/antra
```

The installer asks before it touches your trust store, and tells you how to undo
it. On macOS it uses your login keychain — no sudo. Pin a version for teams and
CI with `ANTRA_VERSION=v0.6.6`.

## What you get

- **Real HTTPS, no warning.** Antra runs a local CA and mints a leaf certificate
  per hostname on SNI. Trust it once, forget it.
- **No hosts file for `.localhost`.** Browsers resolve `*.localhost` natively. No
  edits, no admin rights, works offline.
- **Your process is unchanged.** If it binds a port, Antra can front it. `PORT`,
  `HOST` and `NODE_EXTRA_CA_CERTS` are injected; the rest is your code.
- **HMR works.** Transparent bidirectional WebSocket tunnel — Vite, Next, Rails.
- **Many apps, one address.** Run `antra run` twice. `myapp.localhost` and
  `api.localhost` both resolve; the daemon routes each to whatever port it
  happens to be on.
- **Honest about security.** No telemetry, no cloud, no account. Installing a root
  CA is MITRE ATT&CK T1553.004 and we say so on the page, in the README, and at
  the prompt. `antra trust --remove` undoes it byte-exactly.
- **One native binary.** Rust, `rustls`, `rcgen`. No OpenSSL, no Node runtime, no
  dependency install. macOS, Linux, Windows.

## When not to use it

If someone on another network needs your app, use [ngrok](https://ngrok.com) or
a Cloudflare Tunnel — Antra never sends traffic off your machine and does not
tunnel. If you want a browser-free certificate to hand to one specific server,
use [mkcert](https://github.com/FiloSottile/mkcert).

If you're comparing against tools that do the same job, be aware of
[portless](https://github.com/vercel-labs/portless) — it ships HTTP/2,
framework-aware flag injection, LAN mode and monorepo workspaces today, and it
needs Node 24+. Antra's answer is one native binary that runs for *any*
language and asks before it changes anything. Full side-by-side, sourced and
dated: [`comparison.md`](comparison.md).

If you want your app to feel like production on your laptop, this is the whole
product.

## Try it

```bash
curl -fsSL https://antra.iifelse.com/install.sh | bash
antra run --domain myapp.localhost -- pnpm dev
# open https://myapp.localhost
```

MIT. macOS, Linux, Windows. v0.6.6.

> **Demo placeholder** — record a ~20-second loop using `launch/demo-capture.sh`
> (set up) and `launch/demo-shot-list.md` (exact shots + timings). The command
> and the URL side by side is the whole pitch; a still image can't show it.

> **Competitor naming — decided: name them honestly.** The comparison table —
> which names the direct competitor — ships in the post, the README and the
> landing page. `comparison.md` is the source of truth; keep the three tables in
> sync when a number moves.

---

## Notes for review

| Item | Where it came from |
|---|---|
| Command + output block | `README.md` quick start, verified against the released binary |
| Security framing | `docs/security.md`, `README.md` security table |
| "No sudo" for trust on macOS | `install.sh:227-235`, README service section |
| The port-443 question | `docs/releases/v0.6.6.md` (C27); checked on a real Mac with a real password prompt |
| Windows service caveat | README — the service runs as SYSTEM and uses the SYSTEM CA. Not in the post; it's a docs-level gotcha |
| Linux browser gap | Deliberately not in the post. `check-browsers.sh` reproduces it on every CI run and the release notes state it. Leaving it out of a launch post is a call the maintainer should make knowingly, not by omission |