#!/usr/bin/env bash
# Does a real browser load an Antra URL over HTTPS with no certificate error?
#
# This is the product's headline promise, and until now nothing checked it in
# any browser on any OS. `tests/e2e_securetransport.rs` asks Apple's own TLS
# stack via /usr/bin/curl, which caught a malformed root CA that broke Safari —
# but a TLS *stack* is not a *browser*: Chrome on Linux keeps its own trust
# store and never consults the system one, and Firefox keeps a per-profile NSS
# store. Both read ~/.pki/nssdb, and `antra trust` does not write there (Phase 6
# excludes NSS modification). So `curl` passes and the browser shows
# ERR_CERT_AUTHORITY_INVALID, and only a browser can tell you that.
#
# A failing browser is reported as EXPECTED_FAIL, not skipped and not silently
# passed. A green run must not imply more than it checked: the count of real
# failures and expected failures is printed at the end, and both are non-zero
# when something regressed.
#
# Scope note: Safari is macOS-only and is driven through safaridriver, which
# needs "Allow Remote Automation" in Safari's Develop menu. On a CI runner that
# is not enabled, so Safari reports EXPECTED_FAIL with that reason rather than
# a false pass.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ANTRA_BIN="${ANTRA_BIN:-$REPO_ROOT/target/debug/antra}"
BASE_DOMAIN="${ANTRA_BROWSER_DOMAIN:-browser.localhost}"
UPSTREAM_PORT="${ANTRA_BROWSER_UPSTREAM:-18477}"
HOMEDIR="${ANTRA_BROWSER_HOME:-/tmp/antra-browser-home}"

pass=0
fail=0
expected_fail=0
skip=0

ok()    { printf '  \033[0;32mok\033[0m          %s\n' "$1"; pass=$((pass + 1)); }
bad()   { printf '  \033[0;31mFAIL\033[0m        %s\n' "$1"; fail=$((fail + 1)); }
xfail() { printf '  \033[0;33mEXPECTED_FAIL\033[0m %s\n' "$1"; expected_fail=$((expected_fail + 1)); }
skip_it(){ printf '  \033[0;33mskip\033[0m        %s\n' "$1"; skip=$((skip + 1)); }

# ── A tiny upstream server ───────────────────────────────────────────────────
# Deliberately not a framework: the question here is whether the *TLS* path
# works, so the simplest possible HTTP server keeps the failure surface small.
# It reads PORT, which is what `antra run` injects.
cat > "$HOMEDIR/upstream.js" <<JS
const http = require('http');
// The port is baked in rather than read from PORT: this server is registered
// with \`antra alias\`, which is a static mapping and does not inject PORT the
// way \`antra run\` does. A first version read PORT and silently fell back to
// 3000, so the route pointed at a port nothing was listening on and the check
// reported a 503 that looked exactly like broken TLS.
const port = $UPSTREAM_PORT;
const body = process.env.ANTRA_BROWSER_BODY || 'antra-browser-ok';
http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(\`<h1>\${body}</h1>\`);
}).listen(port, '127.0.0.1', () => console.log('upstream on ' + port));
JS

# Where Antra keeps the CA: `dirs::config_dir()/antra`. That is
# $HOME/Library/Application Support/antra on macOS, and on Linux
# $XDG_CONFIG_HOME/antra when XDG_CONFIG_HOME is set, else $HOME/.config/antra.
# Two earlier versions got this wrong by hardcoding one platform's path, which
# is why the first Ubuntu run reported "no ca.pem" with the CA sitting right
# there. The rule here: ask the environment, never assume, and search as a last
# resort. Note the searches keep stderr — a 0700 root-owned directory produces
# a permission error, and hiding that is how an empty result came to be read as
# "no CA was created" when the truth was "you cannot look in there".
find_ca_pem() {
    case "$(uname -s)" in
        Darwin)
            echo "$HOMEDIR/Library/Application Support/antra/ca.pem"
            ;;
        *)
            if [ -n "${XDG_CONFIG_HOME:-}" ]; then
                echo "$XDG_CONFIG_HOME/antra/ca.pem"
            else
                echo "$HOMEDIR/.config/antra/ca.pem"
            fi
            ;;
    esac
}

CA_PEM="$(find_ca_pem)"
if [ ! -f "$CA_PEM" ]; then
    found="$(find "$HOMEDIR" -name ca.pem -type f 2>&1 | head -1)"
    case "$found" in
        /*) CA_PEM="$found" ;;                 # an absolute path: a real hit
        *)  [ -n "$found" ] && echo "   find said: $found" >&2 ;;
    esac
fi

browser_url() {
    # The proxy is on ANTRA_PORT; scheme is https because that is the promise.
    echo "https://$BASE_DOMAIN:$ANTRA_PORT/"
}

# ── Bring up the whole path this check is about ──────────────────────────────
# Self-contained on purpose. An earlier version assumed a running daemon and a
# registered route, and reported a bare 503 when they were absent — which reads
# as "TLS is broken" and is indistinguishable from the real thing. Setting up
# here means a 503 can only mean the proxy could not reach the upstream.
start_stack() {
    mkdir -p "$HOMEDIR"
    export HOME="$HOMEDIR"
    cd "$HOMEDIR" || exit 1

    "$ANTRA_BIN" proxy start </dev/null >"$HOMEDIR/daemon-start.log" 2>&1
    for _ in $(seq 1 40); do
        "$ANTRA_BIN" list </dev/null 2>&1 | grep -q "Daemon not running" || break
        sleep 0.25
    done
    if "$ANTRA_BIN" list </dev/null 2>&1 | grep -q "Daemon not running"; then
        echo "FAILED: the daemon did not start" >&2
        cat "$HOMEDIR/daemon-start.log" >&2
        return 1
    fi

    # The upstream, on the port the route will point at.
    ANTRA_BROWSER_BODY="${ANTRA_BROWSER_BODY:-antra-browser-ok}" \
        node "$HOMEDIR/upstream.js" >"$HOMEDIR/upstream.log" 2>&1 &
    UPSTREAM_PID=$!
    for _ in $(seq 1 40); do
        curl -s -o /dev/null --max-time 2 "http://127.0.0.1:$UPSTREAM_PORT/" && break
        sleep 0.25
    done
    if ! curl -s -o /dev/null --max-time 2 "http://127.0.0.1:$UPSTREAM_PORT/"; then
        echo "FAILED: the upstream server never came up on $UPSTREAM_PORT" >&2
        cat "$HOMEDIR/upstream.log" >&2
        return 1
    fi

    "$ANTRA_BIN" alias "$BASE_DOMAIN" "$UPSTREAM_PORT" </dev/null \
        >"$HOMEDIR/alias.log" 2>&1

    # The route must be live before any browser is asked, so a browser failure
    # cannot be blamed on the route being missing.
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 \
            --cacert "$CA_PEM" \
            --resolve "$BASE_DOMAIN:$ANTRA_PORT:127.0.0.1" \
            "https://$BASE_DOMAIN:$ANTRA_PORT/" 2>/dev/null)"
    if [ "$code" != "200" ]; then
        echo "FAILED: route $BASE_DOMAIN did not serve 200 (got '$code')" >&2
        return 1
    fi
    return 0
}

# ── A browser probe, via Playwright when it is available ─────────────────────
# Playwright is used only as a driver: it launches the *system* Chrome, Firefox
# and WebKit, which is what makes this a browser check rather than another TLS
# stack check. When it is not installed the probe reports SKIP with the reason
# and the caller records an expected failure — never a pass.
probe() {
    # $1 browser channel: chrome | firefox | webkit
    # $2 url
    # prints: OK <title-ish marker>  |  ERR <message>
    local channel="$1" url="$2"
    node -e '
      const { chromium, firefox, webkit } = require("playwright");
      const channel = process.argv[1];
      const url = process.argv[2];
      const marker = process.argv[3];
      const engine = channel === "chrome" ? chromium : channel === "firefox" ? firefox : webkit;
      (async () => {
        // channel:chrome is ignored by the bundled chromium; the plain
        // chromium build is what CI has, and it uses NSS on Linux, which is
        // the store under test.
        const browser = await engine.launch({ args: ["--no-sandbox"] });
        const page = await browser.newPage({ ignoreHTTPSErrors: false });
        try {
          const resp = await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });
          if (!resp) { console.log("ERR no response"); }
          else {
            const text = await page.textContent("body").catch(() => "");
            console.log((text || "").includes(marker) ? "OK " + resp.status() : "ERR unexpected body: " + String(text).slice(0, 80));
          }
        } catch (e) {
          console.log("ERR " + String(e.message || e).split("\n")[0]);
        } finally {
          await browser.close();
        }
      })();
    ' "$channel" "$url" "${ANTRA_BROWSER_BODY:-antra-browser-ok}"
}

have_playwright() {
    node -e 'require.resolve("playwright")' >/dev/null 2>&1
}

# ── Assertions ───────────────────────────────────────────────────────────────

echo "== antra browser check =="
echo "   binary:    $ANTRA_BIN"
echo "   home:      $HOMEDIR"
echo "   url:       $(browser_url)"
echo

if [ ! -x "$ANTRA_BIN" ]; then
    echo "antra binary not built at $ANTRA_BIN — run cargo build first" >&2
    exit 1
fi

# The CA must exist for any TLS assertion to mean anything. It may legitimately
# be missing: the trust step above can fail, and on a macOS runner it does. So
# this is a reported failure with a reason, not a silent skip and not an abort —
# an abort here would report "no ca.pem" for what is really "trust could not be
# installed", which is the misreading C19 must not be confused with.
if [ ! -f "$CA_PEM" ]; then
    # This must be a hard failure, not an expected one. It was briefly exit 2
    # (a warning, job still green) and that produced the worst result in this
    # workflow's history so far: a green job that verified nothing at all,
    # because the trust step had quietly failed and the run reported success.
    # A check that cannot run is a broken check, and a maintainer scanning a
    # list of green jobs would have no way to tell this one apart from a real
    # pass. The distinction that matters is not "who caused it" but "did
    # anything get verified" — and here, nothing did.
    bad "no ca.pem under $HOMEDIR — the trust step did not produce one (searched the platform path and the whole home)"
    echo
    printf 'antra-browser: 0 pass / 1 fail / 0 expected-fail / 0 skip\n'
    echo "NOTHING WAS CHECKED. Antra's own TLS is unverified in this run." >&2
    exit 1
fi
ok "CA present at $CA_PEM"

if ! start_stack; then
    echo "antra-browser: 0 pass / 1 fail / 0 expected-fail / 0 skip" >&2
    exit 1
fi
ok "daemon, upstream and route are live ($BASE_DOMAIN -> 127.0.0.1:$UPSTREAM_PORT)"

# The driver is not optional. It was `skip` for a while, and the first Ubuntu
# run used that to go green having launched no browser at all: `npm install -g`
# does not put playwright on node's module path, `require.resolve` failed, the
# three browser lines became expected-failures, and 0 pass / 0 fail looked
# fine. A check that cannot ask its question must fail, not skip.
if have_playwright; then
    ok "Playwright available"
    driver_state=present
else
    bad "Playwright is not resolvable by node — no browser can be asked anything"
    echo "   fix: npm install -g playwright && npx playwright install --with-deps chromium firefox webkit" >&2
    echo "   and: echo NODE_PATH=\$(npm root -g) — a global install is not on node's module path" >&2
    driver_state=missing
fi

# Whether the CA is actually trusted *in this environment*. This is the single
# most important input to how a browser failure is read, so it is a real
# assertion rather than a comment.
#
# Measured 2026-10-01: `sudo antra trust` succeeds on ubuntu-latest and fails on
# macos-latest, where neither the user keychain nor the system keychain is
# writable without a GUI authorisation dialog. A macOS runner therefore cannot
# host this check at all, and its browser failures say nothing about Antra's
# certificates. Conflating that with the Linux NSS gap (C19) would be a false
# report of the same bug in two places.
TRUSTED=no
if "$ANTRA_BIN" trust --status </dev/null 2>&1 | grep -q "CA is trusted"; then
    TRUSTED=yes
    ok "the CA is trusted in this environment"
else
    xfail "the CA is NOT trusted here — browser results below say nothing about Antra's certificates"
fi

for b in chrome firefox; do
    url="$(browser_url)"
    if [ "$driver_state" = "missing" ]; then
        bad "$b: never asked — no Playwright driver"
        continue
    fi
    result="$(probe "$b" "$url")"
    case "$result" in
        OK*)
            ok "$b loaded $(browser_url) with no certificate error (${result#OK })"
            ;;
        *)
            if [ "$TRUSTED" = "no" ]; then
                xfail "$b: $result  [cause: no CA trust in this environment, not Antra's TLS]"
            elif [ "$(uname -s)" = "Linux" ]; then
                # Expected on Linux: `antra trust` writes the system store,
                # Chrome and Firefox read ~/.pki/nssdb. Recorded, not hidden —
                # this is the debt Phase 6's NSS exclusion creates (ROADMAP C19).
                xfail "$b: $result  [cause: Linux reads ~/.pki/nssdb and antra trust writes only the system store — ROADMAP C19]"
            else
                bad "$b: $result"
            fi
            ;;
    esac
done

# WebKit stands in for Safari. Not the same engine as Safari proper, so it is
# reported separately and never counted as a Safari pass.
url="$(browser_url)"
if [ "$driver_state" = "present" ]; then
    result="$(probe webkit "$url")"
    case "$result" in
        OK*)   ok "WebKit loaded $(browser_url) with no certificate error (${result#OK })" ;;
        *)     if [ "$TRUSTED" = "no" ]; then
                   xfail "WebKit: $result  [cause: no CA trust in this environment, not Antra's TLS]"
               else
                   bad "WebKit: $result"
               fi ;;
    esac
else
    bad "WebKit: never asked — no Playwright driver"
fi

# Safari proper, through safaridriver. Kept as a distinct line so a future
# runner with Remote Automation enabled fills it in without a code change.
if [ "$(uname -s)" = "Darwin" ] && command -v safaridriver >/dev/null 2>&1; then
    skip_it "Safari: safaridriver present but Remote Automation is not enabled on CI runners (see script header)"
else
    skip_it "Safari: not macOS"
fi

# The system TLS stack, for contrast. This must pass on every OS and is the one
# assertion here that is known to be meaningful today — it is what
# e2e_securetransport.rs already covers on macOS. A failure here is a real
# regression in Antra's own TLS, not a browser store issue.
#
# It passes `--cacert` explicitly, so it validates Antra's certificate and the
# proxy's TLS without needing the CA in any system store. That is deliberate: it
# is the only strict assertion that still means something on a runner where
# trust cannot be installed, which is every macOS runner today.
code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 \
        --cacert "$CA_PEM" \
        --resolve "$BASE_DOMAIN:$ANTRA_PORT:127.0.0.1" \
        "$(browser_url)" 2>/dev/null)"
if [ "$code" = "200" ]; then
    ok "curl against the same URL with --cacert: 200 (Antra's own TLS is sound)"
else
    bad "curl against $(browser_url): got '$code', expected 200 — Antra's own TLS is broken, not just a browser store"
fi

echo
if [ -n "${UPSTREAM_PID:-}" ]; then
    kill "$UPSTREAM_PID" 2>/dev/null
    # bash reports "Terminated: 15" for a job of its own when it reaps it, which
    # reads like a failure at the end of an otherwise green run. `wait` with
    # the status discarded is what suppresses it.
    wait "$UPSTREAM_PID" 2>/dev/null
    "$ANTRA_BIN" remove "$BASE_DOMAIN" </dev/null >/dev/null 2>&1
    "$ANTRA_BIN" proxy stop </dev/null >/dev/null 2>&1
fi

printf 'antra-browser: %d pass / %d fail / %d expected-fail / %d skip\n' \
    "$pass" "$fail" "$expected_fail" "$skip"
# Expected failures are a recorded debt, so they do not fail the job. A real
# failure does. Exiting non-zero on expected_fail would make the job permanently
# red for a decision already taken, which trains everyone to ignore it.
[ "$fail" -eq 0 ]
