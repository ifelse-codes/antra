# LinkedIn post — soft launch of antra

> **PUBLISHED 2026-10-07.** Live at
> `https://www.linkedin.com/feed/update/urn:li:ugcPost:7513444740139524096/`
> (short link `https://lnkd.in/euBjH4tN`, all three destinations verified 200).
> Posted as text + link — **the video was not attached**; `antra-intro-demo.mp4`
> is the natural follow-up. Comments get replies within the first hour.

> Ready to paste. Facts drawn from `README.md`, `landing/index.html`,
> `launch/launch-post.md` and the published v0.6.6 binary. Video:
> `antra-intro-demo.mp4` (1.7 MB, 36 s) at the repo root.
>
> **How to post it:** open LinkedIn → Start a post → paste the text →
> attach `antra-intro-demo.mp4` as **native video** (not a link; native video
> gets the reach). Put the link in the post, not in a comment. Post Tue–Thu,
> 8–10 am in your timezone. Delete this file's instructions before posting.

---

## Post A — the main one (recommended)

```
Local development still looks like 2009.

I wanted a URL. I got localhost:5173.
I wanted HTTPS. I got a red lock and a "proceed anyway" click.
I wanted two services. I got a spreadsheet of ports.

So the pile grows: edit /etc/hosts, run mkcert, write a Caddyfile, remember
to trust a CA — and then watch HMR break because the proxy doesn't tunnel
WebSockets.

Last month I shipped the fix I've wanted for years.

antra run --domain myapp.localhost -- pnpm dev
→ https://myapp.localhost

No port. No hosts file. No certificate warning. Vite HMR still working.

One native binary. No Node, no OpenSSL, no dependency install. It runs in
front of whatever your dev server already is — Vite, Next, Rails — and stays
out of the way.

The one thing it asks for: your permission before it touches your trust
store. Installing a root CA is a real MITRE ATT&CK technique and I would
rather tell you that on the page than bury it. `antra trust --remove` undoes
it byte-exactly.

MIT. macOS, Linux, Windows. v0.6.6.

If you want a public tunnel, use ngrok or Cloudflare — this never sends a
byte off your machine, by design.

curl -fsSL https://antra.iifelse.com/install.sh | bash

Repo and the honest comparison, including where bigger tools win:
https://github.com/ifelse-codes/antra

#rust #devtools #localdev #https #opensource #developertools
```

---

## Post B — shorter, for a day you don't have attention to spare

```
Shipped: antra — your local app at a real HTTPS URL.

antra run --domain myapp.localhost -- pnpm dev
→ https://myapp.localhost

No port. No /etc/hosts. No certificate warning. HMR intact.

One native binary, MIT, macOS/Linux/Windows. It asks before touching your
trust store, and removes itself byte-exactly.

https://github.com/ifelse-codes/antra

#rust #devtools #localdev #opensource
```

---

## Post C — the comment style (what you asked for)

Short body, then the three labelled lines. Same shape as the chitra comment.

```
Your local app, at a real HTTPS URL.

One command. No ports, no /etc/hosts, no certificate warning — and it asks
before it touches your trust store.

Repo: https://github.com/ifelse-codes/antra
Docs: https://antra.iifelse.com
Install: brew install ifelse-codes/antra/antra
```

**Without brew** (any macOS or Linux box):

```
Repo: https://github.com/ifelse-codes/antra
Docs: https://antra.iifelse.com
Install: curl -fsSL https://antra.iifelse.com/install.sh | bash
```

**As a reply under someone else's post** — drop the video line, keep it four
lines, or just leave the three labelled lines on their own. A three-line
comment under a good post outperforms a link drop.

---

## Posting notes

| Item | Why |
|---|---|
| Lead with the table/pain | LinkedIn folds after ~2 lines — the "localhost:5173" line is the hook, and it is the one people recognise |
| Video native, not linked | A link to a video is a click; a native upload plays in the feed |
| Name ngrok/portless honestly | Antra's edge is *local-only*, one binary, any language. Overclaiming is the fastest way to lose a technical audience |
| No "excited to launch!" | The product is the story. One soft-launch line at most, if you want one |
| Reply in comments within the hour | Early replies carry the post |

## Follow-up post ideas (only if the first lands)

- "Someone asked about Windows as a service" — the SYSTEM-CA caveat, answered
  plainly (it is in the README).
- "What Antra does not do" — no tunnels, no LAN, no monorepo. Short, honest,
  and it is the post people screenshot.
- A short clip of `antra doctor` output — credibility travels on real output.
