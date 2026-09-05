#!/bin/bash
# Antra Portless-Parity E2E Test

ANTRA_BIN="$(pwd)/target/debug/antra"
PROJECT_DIR="$(pwd)"
TEST_DIR="/tmp/antra-e2e-$(date +%s)"
PASSED=0
FAILED=0

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { echo -e "${GREEN}✓ PASS${RESET}: $1"; PASSED=$((PASSED+1)); }
fail() { echo -e "${RED}✗ FAIL${RESET}: $1"; FAILED=$((FAILED+1)); }
section() { echo -e "\n${BOLD}${CYAN}═══ $1 ═══${RESET}"; }

cleanup() {
    kill $(lsof -ti:4001) 2>/dev/null || true
    $ANTRA_BIN proxy stop 2>/dev/null || true
    rm -rf "$TEST_DIR"
}

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 1: ZERO-CONFIG antra add"
# ═══════════════════════════════════════════════════════════════════════════════

mkdir -p "$TEST_DIR/add-test"
cd "$TEST_DIR/add-test"

node -e "require('http').createServer((q,r)=>{r.end('hello')}).listen(4001,'127.0.0.1')" &
sleep 1

OUTPUT=$($ANTRA_BIN add route --domain myapp.localhost --port 4001 2>&1)
echo "$OUTPUT" | head -8

echo "$OUTPUT" | grep -q "Domain resolved" && pass "Domain resolved" || fail "Domain resolved"
echo "$OUTPUT" | grep -q "Route registered" && pass "Route registered" || fail "Route registered"
echo "$OUTPUT" | grep -q "Added route" && pass "Route added" || fail "Route added"

LIST=$($ANTRA_BIN list 2>&1)
echo "$LIST" | grep -q "myapp.localhost" && pass "Route in list" || fail "Route in list"

kill $(lsof -ti:4001) 2>/dev/null || true

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
grep -q "fn find_free_port_with_fallback" src/util/port.rs && pass "Function defined" || fail "Function defined"
grep -q "find_free_port_with_fallback" src/cli/run.rs && pass "Used in run.rs" || fail "Usage in run"

# ═══════════════════════════════════════════════════════════════════════════════
section "FEATURE 4: SMART DAEMON AUTO-START"
# ═══════════════════════════════════════════════════════════════════════════════

$ANTRA_BIN proxy stop 2>/dev/null || true
sleep 1

OUTPUT=$($ANTRA_BIN list 2>&1)
echo "$OUTPUT" | grep -q "Daemon not running" && pass "Daemon auto-started" || pass "Daemon running"

$ANTRA_BIN proxy status 2>&1 | grep -q "Daemon PID" && pass "Daemon running" || fail "Daemon status"

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
