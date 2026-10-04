# Launch post — draft v1

> Voice is yours to change. Marked lines `EDIT:` are the ones I'd expect you to
> rewrite. Facts are drawn from `README.md`, `landing/index.html` and the
> published v0.6.6 binary; nothing here is aspirational.
> Companion doc: [`comparison.md`](comparison.md).

---

## Headline options

1. `antra run --domain myapp.localhost -- pnpm dev` → https://myapp.localhost
2. Your dev server deserves a real URL
3. Stop typing `:5173`

EDIT: pick one. Option 1 is the command itself as the headline — it is the
strongest thing in the repo because it *is* the product.

---

# Draft

**EDIT (title):** Stable HTTPS domains for local development. No ports, no
`/etc/hosts`, no certificate warnings.

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

If you want your app to feel like production on your laptop, this is the whole
product.

## Try it

```bash
curl -fsSL https://antra.iifelse.com/install.sh | bash
antra run --domain myapp.localhost -- pnpm dev
# open https://myapp.localhost
```

MIT. macOS, Linux, Windows. v0.6.6.

EDIT: add a terminal recording or a 20-second GIF here. The command and the URL
side by side is the whole pitch and a still image doesn't show it.

EDIT: decide whether to name portless in public. See `comparison.md` — it's a
direct, honest comparison and the maintainer may prefer to claim the space
rather than name a competitor. My read: name them in the README and the docs,
stay quiet in the post. A reader who compares and finds the comparison missing
loses more trust than one who never heard of them.

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