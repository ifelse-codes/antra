# Mac checks runbook — draft v1

**Status 2026-10-04: Check 2 is CLOSED and green. Check 1 is the only thing
left, and it needs ~1 minute in a Terminal.**

## Why these two

| Check | State | Why it matters |
|---|---|---|
| **1. `curl \| bash` from a fresh `HOME`** | **OPEN.** The block below is written and its baseline is measured; nobody has run it with a real TTY yet | Every new user arrives through this exact command |
| **2. Firefox on a Mac** | ✅ **CLOSED.** Firefox 157.0 loaded an Antra URL with no certificate warning, and the automated leg reports 8 pass / 0 fail / 0 expected-fail / 1 skip | The headline promise is "no certificate warning" |

---

# Check 1 — the installer, from a fresh `HOME`, on a real Mac

## What we are actually testing

Not "does the installer work" — CI already proves that on macOS runners. This is
the **A2 stall**: one run where the download did not finish, cause never
established. So the script is written to **localise a stall to one phase** rather
than to produce a pass/fail. If it completes in ~15 seconds, the stall was
transient. If it hangs past ~60 s, we learn exactly where.

A fresh `HOME` matters: it forces the unprivileged path (`~/.local/bin`) and
gives a clean `~/.config/antra`, so you see what a genuinely new machine sees,
not what your dev box already has.

### Step 1.1 — Start a clean shell

Open Terminal. Nothing else needed; no repo checkout required.

### Step 1.2 — Paste this block and run it

```bash
FRESH="$(mktemp -d /tmp/antra-fresh.XXXXXX)"
echo "fresh HOME: $FRESH"
echo "$FRESH" > /tmp/antra-fresh-home.txt

# Phase timings. Each phase prints its own elapsed time, so a stall names itself.
# The `shift` is load-bearing: without it bash tries to run the label as a
# command and the phase dies before it starts.
phase() { local n="$1"; shift; printf '\n── %s ──\n' "$n"; time "$@"; }

phase fetch-installer \
  curl -fsSL https://antra.iifelse.com/install.sh -o "$FRESH/install.sh"
wc -c < "$FRESH/install.sh" | tr -d ' ' | sed 's/^/bytes: /'

phase run-installer \
  env HOME="$FRESH" PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" \
    bash -c 'curl -fsSL https://antra.iifelse.com/install.sh | bash'

echo
echo "── what landed ──"
ls -l "$FRESH/.local/bin/antra" 2>&1
env HOME="$FRESH" PATH="$FRESH/.local/bin:/usr/bin:/bin" antra --version 2>&1
env HOME="$FRESH" PATH="$FRESH/.local/bin:/usr/bin:/bin" antra doctor 2>&1
echo
echo "clean-up: rm -rf $FRESH"
```

**Baseline, measured 2026-10-04 on this Mac (macOS 26.5.1, arm64), so you can
tell a stall from a slow run:**

| Phase | Expected | Notes |
|---|---|---|
| `fetch-installer` | ~0.5 s | installer is **12,593 bytes** |
| `run-installer` | ~15 s | binary is ~8.8 MB; most of it is the GitHub download |
| `antra --version` | `antra 0.6.5` | anything else means the wrong binary answered |

15 s is not a stall. If `run-installer` goes past ~60 s, that is the A2 stall.

**One trap in the block above, already handled:** the `PATH` given to the
installer deliberately excludes `~/.local/bin`. This Mac has a real
`antra 0.3.0` sitting in `~/.local/bin` (installed 2026-09-11), and if that
directory is on `PATH` then `antra --version` below reports **0.3.0** and you
will think the installer is broken when it is your own machine answering. That
is exactly the mistake a fresh `HOME` is supposed to rule out, so the follow-up
checks pin `PATH` to the fresh directory instead of prepending to `$PATH`.

Also: run this from Terminal or iTerm, **not** a tool that gives bash no TTY.
With no TTY the installer takes the non-interactive branch and prints
`Non-interactive mode — skipping CA install.` — correct and safe, but it means
Step 1.3 never happened and Check 2 has no trusted CA.

### Step 1.3 — Answer the trust prompt

When the installer reaches **Trust Setup**, answer:

> `Install CA into your login keychain (no sudo)? [Y/n]`

- **Press Enter (accept).** That installs the CA into your **real login
  keychain** — the same one Chrome already trusts from the 2026-10-02 check.
  Undo: `antra trust --remove`.
- **Or type `n`.** The install still completes; you just get cert warnings until
  you run `antra trust` yourself.

Accept it — Step 2 needs a trusted CA, and accepting is what a real user does.


### Step 1.4 — Report back

Paste the whole output, including every `── phase ──` block and its `real`/`user`/`sys` line. I need:

1. Did all three phases complete, or did one hang?
2. `bytes:` — the installer's size (a partial fetch would show here)
3. `antra --version` — should be `0.6.5`
4. `antra doctor` — full output, especially any line about the CA or `:443`

### Step 1.5 — Clean up when you're done

```bash
rm -rf "$(cat /tmp/antra-fresh-home.txt)"
```

Keep the fresh `HOME` around until Check 2 finishes if you want to compare.

### If it stalls

Do **not** Ctrl-C immediately — that destroys the evidence. Note which phase was
running, wait 60 s, then interrupt. The phase name is the finding.

Also worth one line in the report: was anything else running (a big `cargo
build`, a Docker pull, Time Machine)? The A2 stall was on a machine under load,
which is exactly the condition we could not reproduce on a CI runner.

---

# Check 2 — Firefox on a Mac

## What we are actually testing

Whether Firefox loads an Antra HTTPS URL **with no certificate warning**, using
a CA trusted in your login keychain. Chrome passed this on 2026-10-02.
Firefox had never been checked on any machine. **Answered below — it is clean.**

Why it might differ: Firefox keeps its own certificate store rather than reading
the system one wholesale. On macOS it does consult the login keychain, but its
own profile store is separate. `antra trust` on macOS writes the login keychain
and **not** Firefox's profile store — the same exclusion that makes Linux
Chrome/Firefox fail today (C19). So macOS Firefox was genuinely unknown.

## ✅ ANSWERED 2026-10-04 — Firefox on macOS is clean

Run against the real `HOME`, so the CA was the one in the real login keychain
and the check asserted `the CA is trusted in this environment` rather than
expected-failing:

```
  ok          CA present at /Users/suman/Library/Application Support/antra/ca.pem
  ok          daemon, upstream and route are live (browser.localhost -> 127.0.0.1:18477)
  ok          Playwright available
  ok          the CA is trusted in this environment
  ok          chrome loaded https://browser.localhost:18997/ with no certificate error (200)
  ok          firefox loaded https://browser.localhost:18997/ with no certificate error (200)
  ok          WebKit loaded https://browser.localhost:18997/ with no certificate error (200)
  skip        Safari: safaridriver present but Remote Automation is not enabled
  ok          curl against the same URL with --cacert: 200

antra-browser: 8 pass / 0 fail / 0 expected-fail / 1 skip
```

**What this proves:** macOS Firefox trusts an Antra certificate after
`antra trust --user-level`. So C19 is genuinely Linux-only. The macOS answer is
clean for all three engines.

**Leg 2 — the real application, also done.** `brew install --cask firefox`
installed **Mozilla Firefox 157.0** into `/Applications`. A route was opened at
`https://ffprobe.localhost:8443` and the real browser rendered the app's page —
`Directory listing for /` — with **no certificate warning**. Verified
objectively rather than by eye, because a headless browser renders the warning
interstitial instead of the page, so the rendered page *is* the assertion:

```bash
/Applications/Firefox.app/Contents/MacOS/firefox --headless \
  --screenshot /tmp/ff-shot.png --window-size=1200,800 \
  "https://ffprobe.localhost:8443/"
```

The screenshot showed the served directory listing, not
*Warning: Potential Security Risk Ahead*. Had trust failed, that interstitial is
exactly what would have been captured.

**So both legs are green: C19 is Linux-only, and this Mac is done.** The gap
this check was opened for is closed.

**How it was run** (note both halves — either one alone reproduces C21, a green
check that launched no browser):

```bash
cd /Users/suman/playground/antra
npm install -g playwright
NODE_PATH="$(npm root -g)" npx playwright install firefox webkit chromium

NODE_PATH="$(npm root -g)" ANTRA_BROWSER_HOME="$HOME" \
  ANTRA_PORT=18997 ANTRA_HTTP_PORT=18996 \
  ANTRA_BIN="$PWD/target/debug/antra" \
  bash .github/scripts/check-browsers.sh
```

Two things had to be right, both of them the C21 trap:

1. **`NODE_PATH` is mandatory.** `npm install -g playwright` does *not* put the
   driver on node's module path. Verified on this Mac: after the global install,
   `node -e 'require.resolve("playwright")'` still fails. Without `NODE_PATH`
   the script's `have_playwright` returns false and every browser line becomes
   an expected-fail — `0 pass / 0 fail`, which looks like success.
2. **`ANTRA_BROWSER_HOME` must be `$HOME`, not a temp dir.** The script mints
   whatever CA its home implies. Point it at a temp home and it mints a CA that
   is in no trust store, so every browser line expected-fails for a reason that
   has nothing to do with Firefox. That is why CI can only assert `curl --cacert`
   and why a local run must use the real home.

### Step 2.1 — Install Firefox

```bash
brew install --cask firefox
```

Or download it from mozilla.org and drag it to Applications. Either is fine.

### Step 2.2 — Stand up a route

```bash
antra trust --status          # should say: CA is trusted via your login keychain
```

If it does not, run `antra trust --user-level` first. On this Mac it already
reports `✓ CA is trusted via your login keychain (user-level, no sudo)`.

Then, in a **second** Terminal tab:

```bash
python3 -m http.server 18991
```

And in the **first** tab:

```bash
antra alias ffprobe.localhost 18991
```

**Read the URL it prints — do not assume the one in this document.** On this
Mac it is `https://ffprobe.localhost:8443`, not `:443`, because ports 443 and 80
are already held by macOS AirPlay Receiver:

```
  ℹ Note: HTTPS on port 8443 (port 443 unavailable — needs sudo or is in use)
  → https://ffprobe.localhost:8443
```

Two options, and either is fine for this check:

- **Use the `:8443` URL as printed.** No sudo, no system change. A port in the
  URL does not change what the browser does with the certificate, which is what
  is under test.
- **Free 443** (System Settings → General → AirDrop & Handoff → AirPlay Receiver
  off) and then `sudo antra proxy start` for the clean `:443` URL. Worth doing
  once, because a launch demo should not have a port in it.

### Step 2.3 — Open it in Firefox ✅ done, clean

```bash
open -a Firefox https://ffprobe.localhost:8443
```

Firefox 157.0 loaded it with no warning. If you ever re-run this by hand, read
the page and not just the URL bar: the padlock should be normal, the app should
render, and there should be no `SEC_ERROR_UNKNOWN_ISSUER`.

### Step 2.4 — The automated version (already done — see above)

This ran on 2026-10-04 with **8 pass / 0 fail / 0 expected-fail / 1 skip**. The
exact invocation, and the two ways it silently proves nothing, are written up
in the ANSWERED section at the top of this check.

Re-run it only if something changes trust, TLS or the installer. When you do,
read three things in the output rather than the exit code:

1. Whether the CA is **trusted in this environment**.
2. Whether **Playwright was found**. If not, every browser line is an
   expected-fail that says so, and `0 pass / 0 fail` looks like success while
   nothing was asked. A check that cannot ask its question must fail.
3. The final count: `N pass / N fail / N expected-fail / N skip`.

### Step 2.5 — Nothing left to report

Check 2 is complete: both legs green, recorded above.

---

# Report template

Copy this, fill it in, send it back.

```
CHECK 1 — fresh-HOME installer
  phases completed:        [ all three / stalled at: ______ ]
  time for run-installer:  [ real 0m__s ]
  installer bytes:         [ ______ ]
  antra --version:         [ ______ ]
  antra doctor CA line:    [ ______ ]
  antra doctor :443 line:  [ ______ ]
  machine was busy?:       [ yes: ______ / no ]

CHECK 2 — CLOSED 2026-10-04. Firefox 157.0, real profile: clean load, no warning.
          Automated leg: 8 pass / 0 fail / 0 expected-fail / 1 skip. Nothing owed.
```

---

## What each outcome means

| Outcome | What it means | What happens next |
|---|---|---|
| C1 completes in ~15 s | The A2 stall was transient; the last gap closes | Nothing further. Publish |
| C1 stalls again, in a named phase | We have the reproduction and the phase | Fix it properly, with the phase as the test |
| C1 runs long but finishes under ~60 s | Slow network, not the A2 stall | No action. 15 s is the measured norm |
| C1 stalls at `fetch-installer` | The network path to the landing domain is the suspect | Landing/CDN investigation, not an installer bug |
| C1 stalls at `run-installer` | Inside the script — download or checksum | Instrument `download_binary`, reproduce |
| C1 stalls at `verify_checksum` | The `.sha256` fetch never returns | Same, and it would explain C26's Linux form too |
| C2 Playwright not found | The check is vacuous (C21 again) | Install it properly, re-run, do not report a pass |
| C2 Safari `skip` | Remote Automation not enabled | Known and separate. Enable it only if you want that leg |

---

## Do not

- **Do not re-open the Linux Chrome/Firefox gap (C19).** Decided 2026-10-01,
  stays closed. This runbook is measuring macOS and may reveal a *new* macOS
  fact; it does not reopen the Linux decision either way.
- **Do not add the manual Safari pass back to "Still owed".** Not planned. The
  safaridriver `skip` above says so on every run.
- **Do not publish anything.** A person presses go.

## Undo, in one line

```bash
antra trust --remove && rm -rf "$(cat /tmp/antra-fresh-home.txt 2>/dev/null)"
```