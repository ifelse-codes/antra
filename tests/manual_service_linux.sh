#!/usr/bin/env bash
# Manual check for ROADMAP C14: `antra service install|status|uninstall` against
# a REAL `systemd --user` manager. Not an e2e suite and not run by CI.
#
# It is destructive to the machine's own Antra service: every scenario starts
# by uninstalling the service and stopping the daemon, and it writes
# ~/.config/systemd/user/antra-proxy.service. Run it on a throwaway box or
# container, never on a machine whose service you care about. Hence the guard.
#
#   ANTRA_SERVICE_VERIFY=1 bash tests/manual_service_linux.sh
#
# Needs a running user manager. In a container that was not booted with
# systemd, see "Running systemd --user in a container" in AGENT.md.
# Ports 18443/18080 must be free. Expect 28 passed, 0 failed; v0.6.1 scores
# 10 passed, 18 failed on the same box, which is how this was shown to be able
# to fail.
set -u
if [ "${ANTRA_SERVICE_VERIFY:-}" != 1 ]; then
  echo "Refusing to run: this uninstalls and reinstalls your Antra service." >&2
  echo "Set ANTRA_SERVICE_VERIFY=1 to confirm." >&2
  exit 2
fi
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
A="${ANTRA_BIN:-$REPO_ROOT/target/debug/antra}"
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
export ANTRA_PORT=18443 ANTRA_HTTP_PORT=18080
UNIT="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/antra-proxy.service"
LEGACY="$HOME/.config/antra/systemd/user/antra-proxy.service"
pass=0; fail=0
ok()  { echo "  PASS $1"; pass=$((pass+1)); }
bad() { echo "  FAIL $1"; fail=$((fail+1)); }
check() { if eval "$2"; then ok "$1"; else bad "$1"; fi; }

reset() {
  $A service uninstall >/dev/null 2>&1
  $A proxy stop >/dev/null 2>&1
  rm -f "$UNIT" "$LEGACY"; systemctl --user daemon-reload; systemctl --user reset-failed 2>/dev/null
}

echo "== 1. fresh install, with a v0.6.1 legacy unit on disk"
reset; mkdir -p "$(dirname "$LEGACY")"; echo "[Unit]" > "$LEGACY"
out=$($A service status); check "status names the legacy unit" 'grep -q "older Antra" <<<"$out"'
check "status says not installed" 'grep -q "not installed" <<<"$out"'
out=$($A service install)
check "install reports started"   'grep -q "Service started" <<<"$out"'
check "unit is on systemd's path" '[ -f "$UNIT" ]'
check "legacy unit removed"       '[ ! -e "$LEGACY" ]'
check "systemd has it enabled"    '[ "$(systemctl --user is-enabled antra-proxy)" = enabled ]'
p1=$(systemctl --user show antra-proxy -p MainPID --value); sleep 12
p2=$(systemctl --user show antra-proxy -p MainPID --value)
check "daemon stays up (same PID after 12s)" '[ "$p1" = "$p2" ] && [ "$p1" != 0 ]'
check "no restarts" '[ "$(systemctl --user show antra-proxy -p NRestarts --value)" = 0 ]'
check "service status says running" '$A service status | grep -q "Service is running"'
check "antra logs sees the service daemon" '$A logs --lines 50 | grep -q "Daemon ready"'

echo "== 2. re-install while running keeps it running"
out=$($A service install)
check "re-install says already running" 'grep -q "already running" <<<"$out"'
check "same PID" '[ "$(systemctl --user show antra-proxy -p MainPID --value)" = "$p2" ]'

echo "== 3. re-install without a user bus keeps the working unit"
before=$(cat "$UNIT")
out=$(env -u XDG_RUNTIME_DIR -u DBUS_SESSION_BUS_ADDRESS $A service install)
check "reports the bus failure" 'grep -q "Failed to reload systemd" <<<"$out"'
check "does not claim it will start on login" '! grep -q "start automatically" <<<"$out"'
check "working unit left byte-identical" '[ "$(cat "$UNIT")" = "$before" ]'
check "service still running" '[ "$(systemctl --user is-active antra-proxy)" = active ]'

echo "== 4. uninstall"
out=$($A service uninstall)
check "uninstall reports it" 'grep -q "Service uninstalled" <<<"$out"'
check "unit file gone" '[ ! -e "$UNIT" ]'
check "systemd forgot it" '[ "$(systemctl --user show antra-proxy -p LoadState --value)" = not-found ]'
check "daemon stopped" '$A proxy status 2>&1 | grep -q "not running"'
check "second uninstall says not installed" '$A service uninstall | grep -q "not installed"'

echo "== 5. fresh install without a user bus leaves nothing"
reset
out=$(env -u XDG_RUNTIME_DIR -u DBUS_SESSION_BUS_ADDRESS $A service install)
check "reports the bus failure" 'grep -q "Failed to reload systemd" <<<"$out"'
check "hints at the cause" 'grep -q "No systemd user session" <<<"$out"'
check "no unit left behind" '[ ! -e "$UNIT" ]'

echo "== 6. a daemon already running outside the service"
reset; $A proxy start >/dev/null 2>&1
out=$($A service install)
check "warns, does not start" 'grep -q "already running outside the service" <<<"$out"'
check "enabled for next login" '[ "$(systemctl --user is-enabled antra-proxy)" = enabled ]'
sleep 7
check "no restart loop" '[ "$(systemctl --user show antra-proxy -p NRestarts --value)" = 0 ]'
$A proxy stop >/dev/null 2>&1; systemctl --user start antra-proxy; sleep 2
check "hand-over command works" '$A service status | grep -q "Service is running"'

reset
echo
echo "C14 verification: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
