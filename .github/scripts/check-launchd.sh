#!/usr/bin/env bash
# `antra service install|uninstall` against the real launchd — ROADMAP C16.
#
#   ANTRA_BIN=./target/debug/antra .github/scripts/check-launchd.sh
#
# macOS only, and only on a throwaway machine such as a CI runner: every
# scenario uninstalls the service, stops the daemon and rewrites
# ~/Library/LaunchAgents/com.antra.proxy.plist.
#
# The property that matters, checked in each scenario: launchd started the
# job once, and the pid launchd supervises is the daemon's. v0.6.2 failed
# both on a macos-latest runner — 7 runs in 60 s, no pid — because the plist
# ran `antra proxy start`, which forks the daemon and exits.
set -u
A="${ANTRA_BIN:-./target/debug/antra}"
LABEL=com.antra.proxy
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"
LOG="$HOME/Library/Application Support/antra/daemon.log"
pass=0
fail=0

ok()    { echo "  PASS $1"; pass=$((pass + 1)); }
bad()   { echo "::error::$1"; echo "  FAIL $1"; fail=$((fail + 1)); }
check() { if eval "$2"; then ok "$1"; else bad "$1"; fi; }

# One field of `launchctl print` for the job: runs, state, pid.
field() {
  launchctl print "$DOMAIN/$LABEL" 2>/dev/null |
    awk -F' = ' -v k="$1" '$1 ~ "^[[:space:]]*" k "$" {print $2; exit}'
}
daemon_pid() { "$A" proxy status 2>/dev/null | awk '/Daemon PID:/{print $3}'; }
loaded()     { launchctl list "$LABEL" >/dev/null 2>&1; }
show()       { sed 's/^/      /'; }

reset() {
  "$A" service uninstall >/dev/null 2>&1
  "$A" proxy stop >/dev/null 2>&1
  launchctl unload "$PLIST" >/dev/null 2>&1
  rm -f "$PLIST"
  mkdir -p "$(dirname "$LOG")"
  : > "$LOG"
  sleep 1
}

watch_job() {
  local t=0
  while [ "$t" -le "$1" ]; do
    echo "      t=${t}s runs=$(field runs) state=$(field state) pid=$(field pid) daemon=$(daemon_pid)"
    if [ "$t" -lt "$1" ]; then sleep 10; fi
    t=$((t + 10))
  done
}

# The C16 property.
supervised() {
  local runs pid d
  runs="$(field runs)"
  pid="$(field pid)"
  d="$(daemon_pid)"
  check "$1: launchd started the job once (runs=${runs:-?})" '[ "${runs:-0}" = 1 ]'
  check "$1: launchd supervises the daemon (job pid=${pid:-none}, daemon pid=${d:-none})" \
    '[ -n "$d" ] && [ "$pid" = "$d" ]'
  check "$1: no relaunch found the daemon already running" '! grep -q "already running" "$LOG"'
}

echo "launchctl manager: $(launchctl managername 2>&1), domain $DOMAIN"

echo "== A. fresh install"
reset
out="$("$A" service install 2>&1)"; echo "$out" | show
check "A: plist is valid (plutil -lint)" 'plutil -lint "$PLIST" >/dev/null'
check "A: install says it started" 'grep -q "Service loaded and started" <<<"$out"'
watch_job 40
supervised A
out="$("$A" service uninstall 2>&1)"; echo "$out" | show
sleep 2
check "A: uninstall unloads the job" '! loaded'
check "A: uninstall stops the daemon" '[ -z "$(daemon_pid)" ]'

echo "== B. a daemon is already running outside the service"
reset
"$A" proxy start >/dev/null 2>&1
out="$("$A" service install 2>&1)"; echo "$out" | show
check "B: install warns instead of loading a second daemon" \
  'grep -q "already running outside the service" <<<"$out"'
check "B: the job is not loaded" '! loaded'
"$A" proxy stop >/dev/null 2>&1
: > "$LOG"
launchctl load -w "$PLIST"
watch_job 20
supervised "B, after the suggested hand-over"

echo "== C. upgrading over a v0.6.2 plist"
reset
"$A" service install >/dev/null 2>&1
launchctl unload "$PLIST"
# v0.6.2's plist is today's without EnvironmentVariables.
python3 - "$PLIST" <<'EOF'
import plistlib, sys
p = sys.argv[1]
d = plistlib.load(open(p, "rb"))
d.pop("EnvironmentVariables", None)
plistlib.dump(d, open(p, "wb"))
EOF
: > "$LOG"
launchctl load -w "$PLIST"
sleep 15
check "C: the v0.6.2 plist leaves the daemon unsupervised (reproduces C16)" \
  '[ -n "$(daemon_pid)" ] && [ "$(field pid)" != "$(daemon_pid)" ]'
out="$("$A" service install 2>&1)"; echo "$out" | show
check "C: install warns about the daemon the old job left behind" \
  'grep -q "already running outside the service" <<<"$out"'
"$A" proxy stop >/dev/null 2>&1
: > "$LOG"
launchctl load -w "$PLIST"
watch_job 20
supervised "C, after the suggested hand-over"

reset
echo
echo "launchd checks: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
