#!/bin/bash
# Antra Portless-Parity E2E Test

# Anchored to the repo via BASH_SOURCE, not `$(pwd)`: this suite was only
# runnable from the repo root, and CI invokes suites by path.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANTRA_BIN="${ANTRA_BIN:-$REPO_ROOT/target/debug/antra}"
PROJECT_DIR="$REPO_ROOT"
TEST_DIR="/tmp/antra-e2e-$(date +%s)"
PASSED=0
FAILED=0

# The throwaway HTTP server the tests point routes at. Tracked by PID so
# cleanup kills only what this script started — `kill $(lsof -ti:4001)` would
# take down whatever else holds 4001, including the developer's own server if
# the bind below failed.
UPSTREAM_PID=""

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { echo -e "${GREEN}✓ PASS${RESET}: $1"; PASSED=$((PASSED+1)); }
fail() { echo -e "${RED}✗ FAIL${RESET}: $1"; FAILED=$((FAILED+1)); }
section() { echo -e "\n${BOLD}${CYAN}═══ $1 ═══${RESET}"; }

start_upstream() {
    node -e "require('http').createServer((q,r)=>{r.end('hello')}).listen(4001,'127.0.0.1')" &
    UPSTREAM_PID=$!
    sleep 1
}

stop_upstream() {
    [ -n "$UPSTREAM_PID" ] && kill "$UPSTREAM_PID" 2>/dev/null || true
    UPSTREAM_PID=""
}

cleanup() {
    stop_upstream
    $ANTRA_BIN proxy stop 2>/dev/null || true
    rm -rf "$TEST_DIR"
}

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 1: ZERO-CONFIG antra add"
# ═══════════════════════════════════════════════════════════════════════════════

mkdir -p "$TEST_DIR/add-test"
cd "$TEST_DIR/add-test"

start_upstream

OUTPUT=$($ANTRA_BIN add route --domain myapp.localhost --port 4001 2>&1)
echo "$OUTPUT" | head -8

echo "$OUTPUT" | grep -q "Domain resolved" && pass "Domain resolved" || fail "Domain resolved"
echo "$OUTPUT" | grep -q "Route registered" && pass "Route registered" || fail "Route registered"
# Was `grep -q "Added route"`, for a line that no longer exists: a previous
# session removed it as redundant (it repeated what "Route registered" and the
# URL line already said — see the comment at src/cli/add.rs). The URL is now
# the only thing the command adds on top of the confirmation.
echo "$OUTPUT" | grep -q "https://myapp.localhost" && pass "Route URL printed" || fail "Route URL printed"

LIST=$($ANTRA_BIN list 2>&1)
echo "$LIST" | grep -q "myapp.localhost" && pass "Route in list" || fail "Route in list"

stop_upstream

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 2: PACKAGE SCRIPT WRAPPING"
# ═══════════════════════════════════════════════════════════════════════════════

mkdir -p "$TEST_DIR/wrap-test"
cd "$TEST_DIR/wrap-test"

cat > package.json << 'EOF'
{
  "name": "my-webapp",
  "scripts": { "dev": "vite", "build": "vite build", "test": "vitest" },
  "dependencies": { "react": "^18.0.0" }
}
EOF

OUTPUT=$($ANTRA_BIN add wrap-script webapp --command "npm run dev" --port 5173 2>&1)
echo "$OUTPUT" | head -5

echo "$OUTPUT" | grep -q "Added script" && pass "Script added" || fail "Script added"
grep -q "antra:webapp" package.json && pass "In package.json" || fail "In package.json"
grep -q '"build"' package.json && pass "Existing preserved" || fail "Existing preserved"
grep -q '"react"' package.json && pass "Dependencies preserved" || fail "Dependencies"

OUTPUT=$($ANTRA_BIN add wrap-script webapp --command "npm run dev" --port 5173 2>&1)
echo "$OUTPUT" | grep -q "already exists" && pass "Duplicate rejected" || fail "Duplicate rejected"

OUTPUT=$($ANTRA_BIN add wrap-script webapp --command "npm run dev" --port 5174 --force 2>&1)
echo "$OUTPUT" | grep -q "Added script" && pass "Force overwrite" || fail "Force overwrite"

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 3: PORT CONFLICT AUTO-RESOLUTION"
# ═══════════════════════════════════════════════════════════════════════════════

cd "$PROJECT_DIR"
grep -q "pub fn is_port_available" src/util/port.rs && pass "is_port_available defined" || fail "is_port_available defined"
grep -q "is_port_available" src/cli/run.rs && pass "Explicit --port honored in run.rs" || fail "Explicit --port in run"
grep -q "Using port" src/cli/run.rs && pass "Port verbatim message" || fail "Port message"

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 4: READ-ONLY COMMANDS NEVER AUTO-START"
# ═══════════════════════════════════════════════════════════════════════════════

$ANTRA_BIN proxy stop 2>/dev/null || true
sleep 1

OUTPUT=$($ANTRA_BIN list 2>&1)
echo "$OUTPUT" | grep -q "Daemon not running" && pass "list reports daemon state" || fail "list reports state"

$ANTRA_BIN proxy status 2>&1 | grep -qi "not running" && pass "list did not auto-start daemon" || fail "list mutated state"

cd "$PROJECT_DIR"
grep -q "fn ensure_daemon" src/cli/mod.rs && pass "ensure_daemon exists" || fail "ensure_daemon"

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 5: CONTINUOUS PORT SYNC"
# ═══════════════════════════════════════════════════════════════════════════════

cd "$PROJECT_DIR"
[ -f src/util/port_watcher.rs ] && pass "port_watcher.rs exists" || fail "port_watcher.rs"
grep -q "pub mod port_watcher" src/util/mod.rs && pass "Module registered" || fail "Module registration"
grep -q "fn watch_port_changes" src/util/port_watcher.rs && pass "Function defined" || fail "Function"
grep -q "port_watcher::watch_port_changes" src/cli/run.rs && pass "Used in run" || fail "Usage"
grep -q "Stdio::piped()" src/cli/run.rs && pass "stdout piped" || fail "stdout piping"

# ═══════════════════════════════════════════════════════════════════════════════
section "INTEGRATION: ALL COMMANDS"
# ═══════════════════════════════════════════════════════════════════════════════

HELP=$($ANTRA_BIN --help 2>&1)
for cmd in run dev add list doctor trust proxy clean alias open remove prune hosts service; do
    echo "$HELP" | grep -q "$cmd" && pass "Command '$cmd'" || fail "Command '$cmd'"
done

# ═══════════════════════════════════════════════════════════════════════════════
section "SUMMARY"
# ═══════════════════════════════════════════════════════════════════════════════

echo ""
echo -e "${GREEN}Passed: $PASSED${RESET}"
echo -e "${RED}Failed: $FAILED${RESET}"
echo ""

cleanup

if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}ALL TESTS PASSED!${RESET}"
    exit 0
else
    echo -e "${RED}${BOLD}SOME TESTS FAILED${RESET}"
    exit 1
fi
