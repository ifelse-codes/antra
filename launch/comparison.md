# Antra vs the alternatives — draft v2

> All third-party facts checked **2026-10-04** against the sources named in each
> section. Anything I could not confirm from a primary source is marked
> `UNVERIFIED` rather than guessed. Re-check before publishing: portless ships
> often.
>
> v2 (2026-10-04): updated for v0.6.6 (C27, the port-443 question) and merged
> with the separate draft that was in `docs/gtm/comparison.md`, so there is one
> comparison.
>
> Feeds: the launch post's "when not to use it", the landing page comparison
> table, and any future README table.

## The short version

There are three different jobs people mean by "give my dev server HTTPS", and
most comparisons conflate them.

| You actually want | Reach a dev server from the internet | Correctly-trusted HTTPS on your own machine | Stable local hostname, no ports |
|---|---|---|---|
| **Tool** | ngrok, Tailscale Funnel, Cloudflare Tunnel | mkcert, Caddy | **portless, Antra** |

Antra is in the third column. It is not trying to replace ngrok, and it is not
trying to replace mkcert. Anyone who tells you their tool does all three is
describing three tools.

## Head-to-head, the two that matter

These two do the same job Antra does. This is the honest comparison.

| | **Antra** | **portless** |
|---|---|---|
| Repo | `ifelse-codes/antra` | `vercel-labs/portless` |
| Stars | 0 (published 2026-10-04) | 12,642 |
| Licence | MIT (declared in `Cargo.toml`) | Apache-2.0 |
| Language / runtime | Rust, single native binary | TypeScript, needs Node.js 24+ |
| Install | `curl \| bash`, or Homebrew | `npm install -g portless` |
| Stable local domain | `https://myapp.localhost` | `https://myapp.localhost` |
| HTTPS | Local CA, per-SNI leaf certs, HTTP/1.1 + redirects | Local CA, per-SNI leaf certs, **HTTP/2** |
| Trust store | macOS login keychain (no sudo), system store on Linux/Windows | system store; `sudo` auto-elevation for :443 on macOS/Linux |
| URL without a port (443) | Asks once (`Use port 443? [Y/n]`), `sudo` once, then the proxy drops root (v0.6.6, C27); say no and you get `:8443` | Yes, auto-`sudo` |
| Chrome/Firefox on Linux | Warn until you run one `certutil` line (C19, by decision) | README names the system store only — likely the same gap (not tested) |
| Language-agnostic | **Yes** — any process that binds a port | **No** — package.json scripts, npm/bun/yarn/pnpm runtimes |
| Framework-aware flag injection | No — Antra passes `PORT`/`HOST` and finds the real port itself | **Yes** — injects `--port`/`--host` for Vite, Astro, Angular, Expo, React Native, and refuses to inject where it would break |
| Multi-app | `antra run` × N | ✓, incl. monorepo workspaces |
| Monorepo | Not built (roadmap) | ✓ `pnpm-workspace.yaml` / `workspaces` |
| LAN / phone testing | Not built (roadmap) | ✓ `--lan` with mDNS, Next `allowedDevOrigins` notes |
| Git worktree URLs | Not built | ✓ branch-name subdomain prefix |
| Tunnel to the internet | No, by design | Yes, via `--ngrok` / `--tailscale` / `--funnel` |
| Tailscale sharing | No | ✓ |
| Wildcard subdomains | No | ✓ `--wildcard`, most-specific-match routing |
| Custom TLD under a domain you own | No | ✓ `--tld dev.example.com` (for strict OAuth redirect URIs) |
| Routes on the request path | in-memory `RwLock<HashMap>` | `~/.portless/routes.json`, read per request |
| Loop detection | `X-Antra-Hops`, 508 after 5 | `508 Loop Detected` |
| Diagnostics | `antra doctor` (CA validity, trust, daemon, ports, recent log errors) | `portless doctor` (Node, state dir, proxy, routes, CA trust, DNS, LAN) |
| Proxy protocol support | HTTP/1.1 + WebSocket | HTTP/1.1, HTTP/2, WebSocket, extended CONNECT (RFC 8441) |
| Safari `.localhost` | `antra hosts` (managed block) | `portless hosts sync` (auto-synced by default) |
| Maturity | v0.6.6, 2 months old, 0 stars | pre-1.0, created 2026-02-15, pushed 2026-10-03 |

**Where portless genuinely wins, and the post should not pretend otherwise:**

- **HTTP/2.** It multiplexes, which matters for Vite/Nuxt serving hundreds of
  unbundled files against the browser's 6-connections-per-host limit. Antra is
  HTTP/1.1. This is a real performance difference on big frontends.
- **Framework-aware injection.** Portless rewrites the command for you when a
  framework ignores `PORT`. Antra instead detects the real port after the fact
  and moves the route (shipped in v0.6.5, C24) — a different fix for the same
  problem, and it only works on macOS and Linux.
- **Everything Antra has on the roadmap.** LAN mode, monorepo workspaces,
  worktree subdomains, wildcard routing, Tailscale. portless has all of it today.
- **Adoption.** 12,642 stars and 431 forks against 0. If someone asks "which
  should I try", the honest answer for a JavaScript shop is: read both. To
  "it's new": lead with the test story — 520 Rust tests, four end-to-end
  suites and a real-browser check on every change.

**Where Antra wins:**

- **Language-agnostic.** `antra run --domain api.localhost -- cargo run`,
  `-- python -m http.server`, `-- go run .`. Portless requires Node 24+ and a
  package.json script. Antra does not care what your app is written in, or
  whether it has a package.json at all.
- **One native binary.** No Node 24+, no npm install, no dependency tree. Install
  is a curl or a brew.
- **No runtime dependency for the proxy.** Rustls, no OpenSSL subprocess per
  certificate.
- **It asks before it changes your machine.** CA trust and port 443 are each a
  `[Y/n]` prompt, `antra trust --remove` undoes trust, and `.localhost` never
  touches `/etc/hosts`. On macOS the CA goes into the login keychain with no
  `sudo`. Port 443 needs your password once, because macOS and Linux only let
  root open it on `127.0.0.1`, and Antra's proxy gives up root as soon as the
  port is open (v0.6.6). portless runs `sudo` on its own and auto-syncs
  `/etc/hosts` by default.
- **It handles the messy cases.** A server with a hardcoded port gets its
  route moved to the real port (C24). A route left by `kill -9` is gone in
  5 s (C25). A CA rotation no longer breaks domains you opened before (C23).
  `antra doctor` prints the fix, not a stack trace.
- **Routes never touch disk on the request path.**
- **MIT** vs Apache-2.0 — relevant only if you intend to embed it.

**Where they are identical:** both give you a stable `.localhost` name, a
warning-free browser after one trust step, per-hostname leaf certs, multi-app
routing, and a `doctor`. The concept is portless's, and portless shipped it
first and in JavaScript, where most people live. Claiming otherwise would be a
lie.

**The defensible positioning** is not "better than portless". It is: *if your
dev server is not a Node app, or you would rather not install Node 24 to run
your proxy, Antra is one binary and it does not care what your stack is.*

*Sources: [portless README](https://github.com/vercel-labs/portless),
[Better Stack's portless guide](https://betterstack.com/community/guides/web-servers/portless/)
— checked 2026-10-04.*

## The other three

### ngrok — for reach, not for localhost

Not a competitor. Wrong tool for this job, and the docs should say so.

| | |
|---|---|
| What it is | Public tunnel. Your dev server gets a URL on the internet |
| Free tier | 1 GB/month data out · 20,000 HTTP requests/month · 5,000 TCP connections/month · 10,000 logs & events/month · 500 webhook verifications/month · 3 traffic identities/month |
| Free rate limits | 4,000 HTTP req/min · 100 TCP conn/min |
| Free account limits | up to 3 online endpoints · 3 concurrent agents · 1 user · **1 development domain** |
| HTTPS | Yes, automatic certificates, included free |
| Free-tier friction | An interstitial warning page on all HTML browser traffic; removable on any paid plan, or via the `ngrok-skip-browser-warning` header, or a non-standard `User-Agent` |
| Endpoint timeout | None on free — endpoints stay online indefinitely |
| Cost | Free tier as above; paid tiers exist |
| Wins when | someone on another network needs your app |
| Loses when | only *you* need your app, and you also need offline work |

*Sources: [Free Plan Limits](https://ngrok.com/docs/pricing-limits/free-plan-limits)
and [Pricing](https://ngrok.com/pricing) — checked 2026-10-04.*

### mkcert — for the certificate, not the proxy

Also not a competitor. mkcert issues trusted certificates and stops. It does not
proxy, does not route, does not pick a port.

| | |
|---|---|
| What it is | Local CA + certificate generator. Nothing else |
| Its own words | "mkcert does not automatically configure servers to use the certificates" |
| Install | `brew install mkcert` (macOS/Linux) · Chocolatey or Scoop (Windows) |
| Latest release | **v1.4.4, 2022-04-26** |
| Licence | BSD-3-Clause |
| Trust stores | macOS system · Windows system · Linux `update-ca-certificates`/`update-ca-trust`/`trust` · **Firefox (macOS and Linux only)** · Chrome/Chromium · Java |
| Linux prerequisite | `libnss3-tools` for `certutil`, else Firefox is not covered |
| Node caveat | Node ignores the system store — you must set `NODE_EXTRA_CA_CERTS` yourself |
| Wins when | one server, one hostname, you want to configure the proxy yourself |
| Loses when | you wanted routing, multi-app, HMR, or port management — that's Caddy/nginx config on top |

*Sources: [README](https://github.com/FiloSottile/mkcert),
[v1.4.4 release](https://api.github.com/repos/FiloSottile/mkcert/releases/latest) — checked 2026-10-04.*

**Note worth keeping:** mkcert covers Firefox on macOS and Linux but **not**
Windows. Antra's known gap is the mirror image — Chrome/Firefox on Linux. Same
class of problem, opposite corner of the matrix, and it is stated in Antra's
release notes rather than hidden.

### Caddy + mkcert — the config-file answer

`caddy trust` installs a root CA into local trust stores; a three-line Caddyfile
does the proxying. Very good, and it's the most honest competitor for people who
like config files. Costs you: a config file to maintain, one upstream port per
service written down by hand, and the HMR/WebSocket config on you. Antra takes
the same result as a command.

*Source: [Caddy automatic HTTPS](https://caddyserver.com/docs/automatic-https)
— checked 2026-10-04.*

### Tailscale Funnel — reach, with your own tailnet

| | |
|---|---|
| What it is | Public exposure of a local service through Tailscale's relay network |
| Cost | Available on all Tailscale plans; currently in beta |
| Requirements | Tailscale v1.38.3+, MagicDNS, HTTPS enabled for the tailnet, a `funnel` node attribute in the tailnet policy |
| Limits | `*.ts.net` names only · ports 443, 8443, 10000 only · TLS only · subject to non-configurable bandwidth limits |
| Trust | Real public certificates — no local CA, no warning on any machine |
| Wins when | you want a real public URL with a real CA, already run Tailscale |
| Loses when | you wanted a *local* stable name |

*Source: [Tailscale Funnel docs](https://tailscale.com/docs/features/tailscale-funnel)
— checked 2026-10-04.*

### Cloudflare Tunnel

Not researched in this pass — it belongs to the ngrok column (public reach) and
adds nothing to the local-HTTPS comparison. `UNVERIFIED`, deliberately omitted
rather than filled in from memory.

## Suggested one-liners

- **"portless, without Node."** — sharpest, but it defines Antra by a rival.
- **"Real HTTPS for any local dev server. One binary. Asks before it changes anything."**
- **"Stop typing `localhost:5173`."** — the README's own hook, still good.

## Where the old table is wrong

The README and the landing page both carry a comparison table with an **empty
cell** under `localhost:port` → "One-command UX", and no portless column at all.
Two problems:

1. **The direct competitor is missing.** portless does the same job, has 12,642
   stars, and was created 2026-02-15 — after Antra's `docs/research/portless.md`
   first noted it. A reader who tries both will find Antra's table did not
   mention the alternative.
2. **"Stable local domain: Random / paid" for ngrok** is right but misleading in
   context — it suggests ngrok can't do stable domains, when the real difference
   is that ngrok's are *public* domains.

A replacement table is at the bottom of this file. It should be one table, and it
should include portless.

## What I did not verify

- Cloudflare Tunnel pricing/limits — omitted rather than recalled.
- Whether portless's `certutil`-on-Windows path covers Firefox on Windows. Its
  README documents Windows as system-store only; I did not test it.
- Whether Antra's HTTP/1.1-only limitation is documented anywhere user-facing.
  It is not. If portless's HTTP/2 is a fair selling point, Antra's absence of it
  should be stated too, or a reader will find it in a GitHub issue instead.

## Proposed replacement table

For `README.md` and `landing/index.html`.

| | `localhost:port` | mkcert + Caddy | ngrok | portless | **Antra** |
|---|---|---|---|---|---|
| Local stable domain | — | Manual | n/a (public) | ✓ | ✓ |
| HTTPS, no warning | — | Manual | ✓ | ✓ | ✓ (Linux Chrome/Firefox: one `certutil` line) |
| Works offline | ✓ | ✓ | ✗ | ✓ | ✓ |
| Cloud account | — | — | Required | — | — |
| Traffic leaves your machine | — | — | **Yes** | — | **Never** |
| Non-Node dev servers | ✓ | ✓ | ✓ | **✗** | ✓ |
| Runtime to install | — | Caddy | ngrok | **Node 24+** | **none** |
| HMR / WebSocket | ✓ | Config-dependent | ✓ | ✓ | ✓ |
| Monorepo workspaces | DIY | Config file | Extra tunnels | ✓ | *Roadmap* |
| LAN / phone testing | DIY | Config file | ✓ | ✓ | *Roadmap* |
| HTTP/2 | — | ✓ | ✓ | ✓ | **✗** |
| Needs sudo | — | For :443 | — | For :443, runs it itself | Asks once, for :443 only; `n` keeps `:8443` |

*Why 443 needs `sudo` at all: non-root may bind ports below 1024 on macOS only
on `0.0.0.0`, not `127.0.0.1`
([Apple forums](https://developer.apple.com/forums/thread/674179),
[Caddy forum](https://caddy.community/t/how-is-caddy-able-to-bind-to-port-80-and-443-on-macos-without-root/15293)).
Antra binds loopback only, on purpose.*
