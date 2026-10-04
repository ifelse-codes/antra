# Demo shot list — ~20-second loop

Run `bash launch/demo-capture.sh` first; it prints the exact command. Record at
~1200×800, no audio needed. Keep the whole loop under 25 seconds.

| # | Time | Shot | What to show |
|---|---|---|---|
| 1 | 0:00 | Terminal | Type `antra run --domain myapp.localhost -- python3 -m http.server 5173 --directory …` (or the Vite variant). Hit Enter. |
| 2 | 0:03 | Terminal | The four `✓` lines roll in: *Domain resolved*, *Proxy ready*, *HTTPS ready*, *Route registered*. |
| 3 | 0:06 | Terminal | The URL prints — `→ https://myapp.localhost`. **No port number.** |
| 4 | 0:08 | Browser | Paste the URL, hit Enter. Page loads with a **green lock**, no "proceed anyway" click. |
| 5 | 0:12 | Browser | Show the address bar close-up: `https://myapp.localhost` with the lock, so it's readable on a small card. |
| 6 | 0:16 | Browser | (Vite variant only) Edit a line in the page, watch HMR refresh it — proves the WebSocket tunnel. |
| 7 | 0:20 | Terminal | Ctrl+C. The daemon keeps the domain; the app stops cleanly. |

## Capture tips

- The single most persuasive frame is **#4**: the green lock at a bare
  `*.localhost` URL. If you record nothing else, record that.
- Keep shot 5 steady for a full second — it's the one people screenshot.
- No narration. A caption overlay "one command → real HTTPS URL" is enough.
- Trim dead air at the front and back; aim for 20s, hard cap 25s.
