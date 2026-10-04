# Antra vs portless, Caddy, mkcert and ngrok

> GTM draft, 2026-10-04. Facts about other tools come from their own READMEs
> and docs, fetched on that date (sources at the bottom). Anything not read
> from a source says so. Re-check before publishing: portless ships often.

## The one thing to know first

**portless has caught up.** The research note in `docs/research/portless.md`
describes an early portless: no TLS by default, a Node-only tool. The current
one (v0.15.x, ~12.6k stars, pre-1.0) has HTTPS + HTTP/2 by default, trusts its
CA on first run, takes port 443 by asking for `sudo`, runs on Windows, and adds
LAN, Tailscale and ngrok sharing, monorepo config and git-worktree subdomains.

So "stable HTTPS `.localhost` URLs" is no longer a difference on its own. The
differences that are left are below, and they are real.

## At a glance

| | **Antra** | portless | Caddy | mkcert | ngrok |
|---|---|---|---|---|---|
| What it is | Local HTTPS proxy that runs your command | Local HTTPS proxy that runs your command | General web server | Certificate maker | Public tunnel |
| Install needs | Nothing (one Rust binary) | **Node.js 24+** (`npm i -g`) | Nothing (one Go binary) | Nothing (+ `certutil` on Linux) | Account |
| One command per app | `antra run --domain app.localhost -- pnpm dev` | `portless app pnpm dev` | `caddy reverse-proxy --from app.localhost --to :3000` | No (certs only) | `ngrok http 3000` |
| Picks the app's port for you | Yes | Yes (4000–4999) | No — you pass it | n/a | No |
| HTTPS with no warning | After a `[Y/n]` prompt | Yes, trusts CA on first run | Yes, trusts CA on first use | Yes (`mkcert -install`) | Yes (public cert) |
| URL without a port | **Only with sudo today** — else `:8443` (C27) | Yes, auto-`sudo` | Yes, needs rights for 443 | n/a | Yes |
| Chrome/Firefox on Linux | Warn (C19, by decision) | README names the system store only — likely the same gap (not tested) | Not checked | **Yes** (NSS) | Yes |
| Server that ignores `PORT` | Finds the real port, moves the route (C24) | Injects `--port` for known frameworks | n/a | n/a | n/a |
| Route after `kill -9` | Removed within 5 s (C25) | Not documented; `prune` for orphans | n/a | n/a | n/a |
| `/etc/hosts` | Never for `.localhost`; asks for others | **Auto-syncs by default** (Safari) | No | No | No |
| Windows | Yes | Yes | Yes | Yes | Yes |
| Share on LAN / Tailscale / public | No (after launch, on demand) | Yes | Manual | No | **Yes — that is its job** |
| Monorepo / worktrees | `antra.toml` per app | Yes | Caddyfile | n/a | n/a |
| Works offline | Yes | Yes | Yes | Yes | No |
| Telemetry / account | None | None found | None | None | Account |
| Licence | MIT | Apache-2.0 | Apache-2.0 | BSD-3 | Proprietary |

## Where Antra wins

1. **No Node.** One binary, `curl | bash` or Homebrew. portless needs Node 24+,
   which a Python, Go, Rust, Ruby or PHP developer may not have, or may have
   pinned to an older version for their own project.
2. **It asks before it touches your machine.** CA trust is a `[Y/n]` prompt,
   `antra trust --remove` undoes it, and `.localhost` never touches
   `/etc/hosts`. portless auto-syncs `/etc/hosts` by default and auto-elevates
   with `sudo`.
3. **It handles the messy cases.** A server with a hardcoded port gets its
   route moved to the real port (C24). A route left by `kill -9` is gone in
   5 s (C25). A CA rotation no longer breaks domains you opened before (C23).
   `antra doctor` prints the fix, not a stack trace.
4. **Against Caddy:** Caddy is a full web server and does HTTPS on
   `.localhost` well, but you start your app, pick its port and keep the two
   in sync yourself. Antra runs your command, gives it a port, and removes the
   route when it exits.
5. **Against mkcert:** mkcert makes certificates; you still wire up a server,
   a port and a hostname for each app.
6. **Against ngrok:** different job. ngrok puts your app on the internet for
   someone else. Antra makes it feel like production on your own laptop, with
   no account and no network.

## Where Antra loses today — say it before a commenter does

| Gap | Honest answer |
|---|---|
| URL has `:8443` unless you use `sudo` (C27) | Fix decided 2026-10-04: ask once on first run, then use port 443. Not built yet. |
| Chrome/Firefox on Linux warn (C19) | By decision. Workaround in the release notes (`certutil`). mkcert covers this; portless's README suggests it does not. |
| No LAN / Tailscale / public sharing | Out of scope until users ask. Point to ngrok/Cloudflare Tunnel. |
| No monorepo or worktree routing | Same. |
| Small, new project | True. Lead with the test story: 500 Rust tests, four e2e suites and a real-browser check on every change. |

## Suggested one-liners

- **"portless, without Node."** — sharpest, but it defines Antra by a rival.
- **"Real HTTPS for any local dev server. One binary. Asks before it changes anything."**
- **"Stop typing `localhost:5173`."** — the README's own hook, still good.

## README fix needed

The README's *Antra vs the usual suspects* table:

- leaves out portless, the closest rival;
- says ngrok's domain is "Random / paid" — the free plan now includes one
  static domain;
- says "One-command UX: Yes" only for Antra, which portless and Caddy now
  match.

Replace it with a trimmed version of the table above once C27 is built, so the
"URL without a port" row can say yes.

## Sources (fetched 2026-10-04)

- portless README: https://github.com/vercel-labs/portless
- portless version and trend notes: https://betterstack.com/community/guides/web-servers/portless/
- mkcert README: https://github.com/FiloSottile/mkcert
- Caddy automatic HTTPS (local CA, trust prompt): https://caddyserver.com/docs/automatic-https
- ngrok free plan limits: https://ngrok.com/docs/pricing-limits
- macOS port binding (non-root may bind < 1024 only on `0.0.0.0`, not
  `127.0.0.1`): https://developer.apple.com/forums/thread/674179 and
  https://caddy.community/t/how-is-caddy-able-to-bind-to-port-80-and-443-on-macos-without-root/15293
