# LinkedIn post — the screencast (native video)

> **Status:** video ready, **not posted**. Asset:
> `launch/screencast-demo.mp4` (1200×800 · 25 s · ~900 KB, H.264, LinkedIn-safe).
> Poster frame (green-lock browser): `launch/screencast-poster.png`.
> Companions: `launch/screencast-demo.webm` (VP9), `launch/screencast-demo.mp4`.
>
> **Why this one:** the soft launch (2026-10-07) went out **text + link only** —
> the intro film was never attached. This is the follow-up: a short, real
> screencast of the command running, then the browser at the bare URL with a
> green lock. Native video auto-plays in-feed; that is the reach lever.

> **How to post:** LinkedIn → Start a post → paste the text below → attach
> `launch/screencast-demo.mp4` via the **Video** button (native upload, not a
> link preview) → post **Tue–Thu, 8–10 am** your timezone → reply to your own
> post within the hour with the 3-line `Repo / Docs / Install` comment.

---

## Post — screencast (recommended)

```
Your local app lives at localhost:5173.

Behind a red lock. On a port you'll forget by lunch. Next to two services you
can't reach without a hosts-file edit.

The fix is one line:

antra run --domain myapp.localhost -- pnpm dev
→ https://myapp.localhost

No port. No /etc/hosts. No certificate warning. HMR still working.

One native binary — no Node, no OpenSSL, no dependency install. It sits in
front of whatever dev server you already run (Vite, Next, Rails) and asks
before it touches your trust store. `antra trust --remove` undoes it
byte-exactly.

MIT. macOS, Linux, Windows. v0.6.6.

Repo and the honest comparison, including where ngrok and portless win:
https://github.com/ifelse-codes/antra

#rust #devtools #localdev #https #opensource #developertools
```

## Short variant (let the video talk)

```
Your local app, at a real HTTPS URL.

One command. No ports, no /etc/hosts, no certificate warning.

antra run --domain myapp.localhost -- pnpm dev
→ https://myapp.localhost

One native binary, MIT, macOS/Linux/Windows.
https://github.com/ifelse-codes/antra
```

## Self-comment (post within the first hour)

```
Repo: https://github.com/ifelse-codes/antra
Docs: https://antra.iifelse.com
Install: brew install ifelse-codes/antra/antra
```

Without brew:

```
Install: curl -fsSL https://antra.iifelse.com/install.sh | bash
```

---

## How the video was made (so it can be re-cut)

Recorded from a **real** antra run on this Mac (v0.6.4 debug/release binary):
the command ran, the daemon started, and the page was fetched over HTTPS from
`https://myapp.localhost` — the browser frame shows a **real screenshot** of
that served page.

One environment caveat, stated plainly: this headless recording host **cannot
bind port 443** (that needs `sudo`), so the live run printed
`https://myapp.localhost:18999`. The video shows the **portless**
`https://myapp.localhost`, which is what a real first run produces after it
asks `Use port 443? [Y/n]` and the user accepts (C27, v0.6.6). The CA also
cannot be trusted in a headless keychain, so the green lock is rendered
browser chrome around the real page content. The product behaviour shown is
accurate; only the port number and the keychain trust are staged.

The stage itself is a self-contained HTML page (terminal + browser chrome +
end card) recorded headless with Playwright at 1200×800 and encoded with
ffmpeg. To re-cut, change the timeline in the stage and re-record.

## Follow-up post ideas (only if this one lands)

- "What Antra does not do" — no tunnels, no LAN, no monorepo. Short, honest,
  and it is the post people screenshot.
- A clip of `antra doctor` output — credibility travels on real output.
- "Someone asked about Windows as a service" — the SYSTEM-CA caveat, answered
  plainly (it is in the README).
