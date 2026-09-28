#!/bin/bash
set -e

# Antra Portless-Parity Features E2E Test Script
# Tests all 5 new features implemented to close gap with Vercel's portless

# Tests `cd` into scratch dirs under $TEST_DIR, so resolve both paths against
# the repo up front: $ANTRA_BIN to reach the binary, $REPO_ROOT for the
# source-level assertions.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANTRA_BIN="${ANTRA_BIN:-$REPO_ROOT/target/debug/antra}"
TEST_DIR="/tmp/antra-portless-tests"
RESULTS_FILE="/tmp/antra-portless-test-results.txt"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

pass_count=0
fail_count=0

log_pass() {
    # `((x++))` exits 1 when x was 0 (it evaluates the *old* value), which
    # under `set -e` kills the suite on its very first passing assertion.
    # Assignment always succeeds, and the counter is the same.
    echo -e "${GREEN}✓ PASS${RESET}: $1" | tee -a "$RESULTS_FILE"
    pass_count=$((pass_count + 1))
}

log_fail() {
    echo -e "${RED}✗ FAIL${RESET}: $1" | tee -a "$RESULTS_FILE"
    fail_count=$((fail_count + 1))
}

log_section() {
    echo -e "\n${BOLD}${CYAN}═══════════════════════════════════════════${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${BOLD}${CYAN}  $1${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${BOLD}${CYAN}═══════════════════════════════════════════${RESET}\n" | tee -a "$RESULTS_FILE"
}

cleanup() {
    # Kill any background processes
    pkill -f "antra proxy start" 2>/dev/null || true
    # Was `pkill -f "node.*test"`, which matches any node process whose
    # command line contains "test" — a developer's Jest/Vitest/watch run, not
    # just this suite's. Scope it to processes rooted in $TEST_DIR.
    pkill -f "$TEST_DIR" 2>/dev/null || true
    rm -rf "$TEST_DIR"
    # $RESULTS_FILE is this run's record and the artifact the caller reads;
    # setup() clears the previous one instead.
}

# Echo a port nothing is listening on. Hardcoding 4001 works until a
# developer's own server is on it, and then the suite fails for reasons that
# have nothing to do with Antra.
free_port() {
    local p
    for p in $(seq 45000 45040); do
        if ! (exec 3<>"/dev/tcp/127.0.0.1/$p") 2>/dev/null; then
            echo "$p"
            return 0
        fi
    done
    echo 45099
}

setup() {
    rm -rf "$TEST_DIR"
    mkdir -p "$TEST_DIR"
    echo "Antra Portless-Parity E2E Test Results - $(date)" > "$RESULTS_FILE"
}

wait_for_daemon() {
    for i in $(seq 1 30); do
        if $ANTRA_BIN proxy status 2>/dev/null | grep -q "running"; then
            return 0
        fi
        sleep 0.5
    done
    return 1
}

# ═══════════════════════════════════════════════════════════════════════════════
# FEATURE 1: PORT CONFLICT AUTO-RESOLUTION
# ═══════════════════════════════════════════════════════════════════════════════

test_port_conflict_help() {
    log_section "Feature #29: Port Conflict Auto-Resolution"

    output=$($ANTRA_BIN run --help 2>&1 || true)

    if echo "$output" | grep -q "\-\-port"; then
        log_pass "Run command has --port flag"
    else
        log_fail "Run command has --port flag"
    fi
}

test_port_conflict_code() {
    log_section "Feature #29: Port Conflict - explicit port honored verbatim"

    if grep -q "pub fn is_port_available" "$REPO_ROOT/src/util/port.rs"; then
        log_pass "is_port_available function exists"
    else
        log_fail "is_port_available function exists"
    fi

    if grep -q "Using port" "$REPO_ROOT/src/cli/run.rs"; then
        log_pass "run reports verbatim port use"
    else
        log_fail "run reports verbatim port use"
    fi
}

test_port_conflict_used_in_run() {
    log_section "Feature #29: Port Conflict - Used in run.rs"

    if grep -q "is_port_available" "$REPO_ROOT/src/cli/run.rs"; then
        log_pass "is_port_available used in run.rs"
    else
        log_fail "is_port_available used in run.rs"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# FEATURE 2: SMART DAEMON AUTO-START
# ═══════════════════════════════════════════════════════════════════════════════

test_smart_daemon_code() {
    log_section "Feature #27: Smart Daemon Auto-Start"

    if grep -q "fn ensure_daemon" "$REPO_ROOT/src/cli/mod.rs"; then
        log_pass "ensure_daemon function exists in mod.rs"
    else
        log_fail "ensure_daemon function exists in mod.rs"
    fi
}

test_smart_daemon_in_list() {
    log_section "Feature #27: Read-only list never auto-starts daemon"

    if grep -B2 "list::execute" "$REPO_ROOT/src/cli/mod.rs" | grep -q "ensure_daemon"; then
        log_fail "list must not auto-start daemon"
    else
        log_pass "list does not auto-start daemon"
    fi
}

test_smart_daemon_in_alias() {
    log_section "Feature #27: Smart Daemon - auto-start in alias"

    if grep -B2 "alias::execute" "$REPO_ROOT/src/cli/mod.rs" | grep -q "ensure_daemon"; then
        log_pass "ensure_daemon called for alias command"
    else
        log_fail "ensure_daemon called for alias command"
    fi
}

test_smart_daemon_in_open() {
    log_section "Feature #27: open never auto-starts daemon"

    if grep -B2 "open::execute" "$REPO_ROOT/src/cli/mod.rs" | grep -q "ensure_daemon"; then
        log_fail "open must not auto-start daemon"
    else
        log_pass "open does not auto-start daemon"
    fi
}

test_smart_daemon_in_remove() {
    log_section "Feature #27: remove never auto-starts daemon"

    if grep -B2 "println.*Removing route" "$REPO_ROOT/src/cli/mod.rs" | grep -q "ensure_daemon"; then
        log_fail "remove must not auto-start daemon"
    else
        log_pass "remove does not auto-start daemon"
    fi
}

test_smart_daemon_in_prune() {
    log_section "Feature #27: prune never auto-starts daemon"

    if grep -B2 "prune::execute" "$REPO_ROOT/src/cli/mod.rs" | grep -q "ensure_daemon"; then
        log_fail "prune must not auto-start daemon"
    else
        log_pass "prune does not auto-start daemon"
    fi
}

test_smart_daemon_list_no_daemon() {
    log_section "Feature #27: list reports state without mutating it"

    # Stop any running daemon first
    $ANTRA_BIN proxy stop 2>/dev/null || true
    sleep 1

    output=$($ANTRA_BIN list 2>&1 || true)

    if echo "$output" | grep -q "Daemon not running"; then
        log_pass "list reports daemon state"
    else
        log_fail "list reports daemon state"
    fi

    status_out=$($ANTRA_BIN proxy status 2>&1 || true)
    if echo "$status_out" | grep -qi "not running"; then
        log_pass "list did not auto-start daemon"
    else
        log_fail "list did not auto-start daemon"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# FEATURE 3: ZERO-CONFIG `antra add` COMMAND
# ═══════════════════════════════════════════════════════════════════════════════

test_add_command_help() {
    log_section "Feature #28: Zero-Config antra add"

    output=$($ANTRA_BIN add --help 2>&1 || true)

    if echo "$output" | grep -q "Add a route"; then
        log_pass "add command has description"
    else
        log_fail "add command has description"
    fi

    if echo "$output" | grep -q "route"; then
        log_pass "add has route subcommand"
    else
        log_fail "add has route subcommand"
    fi

    if echo "$output" | grep -q "wrap-script"; then
        log_pass "add has wrap-script subcommand"
    else
        log_fail "add has wrap-script subcommand"
    fi
}

test_add_route_help() {
    log_section "Feature #28: antra add route --help"

    output=$($ANTRA_BIN add route --help 2>&1 || true)

    if echo "$output" | grep -q "\-\-domain"; then
        log_pass "add route has --domain flag"
    else
        log_fail "add route has --domain flag"
    fi

    if echo "$output" | grep -q "\-\-port"; then
        log_pass "add route has --port flag"
    else
        log_fail "add route has --port flag"
    fi

    if echo "$output" | grep -q "\-\-tld"; then
        log_pass "add route has --tld flag"
    else
        log_fail "add route has --tld flag"
    fi
}

test_add_route_no_daemon() {
    log_section "Feature #28: antra add route (no daemon)"

    $ANTRA_BIN proxy stop 2>/dev/null || true
    sleep 1

    output=$($ANTRA_BIN add route --domain test-add.localhost --port "$(free_port)" 2>&1 || true)

    if echo "$output" | grep -q "Daemon not running"; then
        log_pass "add route detects daemon not running"
    else
        # `add` auto-starts the daemon (ROADMAP #27), so a stopped daemon
        # still produces "Daemon not running, starting it...". This `else`
        # used to log a pass, which meant the assertion could never fail.
        log_fail "add route detects daemon not running"
    fi
}

test_add_code_exists() {
    log_section "Feature #28: add.rs exists"

    if [ -f "$REPO_ROOT/src/cli/add.rs" ]; then
        log_pass "src/cli/add.rs exists"
    else
        log_fail "src/cli/add.rs exists"
    fi
}

test_add_module_registered() {
    log_section "Feature #28: add module registered"

    if grep -q "pub mod add" "$REPO_ROOT/src/cli/mod.rs"; then
        log_pass "add module registered in mod.rs"
    else
        log_fail "add module registered in mod.rs"
    fi
}

test_add_command_variant() {
    log_section "Feature #28: Add variant in Commands enum"

    if grep -q "Add(add::AddArgs)" "$REPO_ROOT/src/cli/mod.rs"; then
        log_pass "Add variant in Commands enum"
    else
        log_fail "Add variant in Commands enum"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# FEATURE 4: CONTINUOUS PORT SYNC
# ═══════════════════════════════════════════════════════════════════════════════

test_port_watcher_exists() {
    log_section "Feature #25: Continuous Port Sync"

    if [ -f "$REPO_ROOT/src/util/port_watcher.rs" ]; then
        log_pass "src/util/port_watcher.rs exists"
    else
        log_fail "src/util/port_watcher.rs exists"
    fi
}

test_port_watcher_module() {
    log_section "Feature #25: port_watcher module registered"

    if grep -q "pub mod port_watcher" "$REPO_ROOT/src/util/mod.rs"; then
        log_pass "port_watcher module registered"
    else
        log_fail "port_watcher module registered"
    fi
}

test_port_watcher_function() {
    log_section "Feature #25: watch_port_changes function"

    if grep -q "fn watch_port_changes" "$REPO_ROOT/src/util/port_watcher.rs"; then
        log_pass "watch_port_changes function exists"
    else
        log_fail "watch_port_changes function exists"
    fi
}

test_port_watcher_patterns() {
    log_section "Feature #25: Port detection patterns"

    if grep -q "PORT_PATTERNS" "$REPO_ROOT/src/util/port_watcher.rs"; then
        log_pass "PORT_PATTERNS constant exists"
    else
        log_fail "PORT_PATTERNS constant exists"
    fi

    if grep -q "listening on" "$REPO_ROOT/src/util/port_watcher.rs"; then
        log_pass "Has 'listening on' pattern"
    else
        log_fail "Has 'listening on' pattern"
    fi
}

test_port_watcher_used_in_run() {
    log_section "Feature #25: port_watcher used in run.rs"

    if grep -q "port_watcher::watch_port_changes" "$REPO_ROOT/src/cli/run.rs"; then
        log_pass "port_watcher used in run.rs"
    else
        log_fail "port_watcher used in run.rs"
    fi
}

test_port_watcher_stdout_capture() {
    log_section "Feature #25: stdout captured for port watching"

    if grep -q "stdout(std::process::Stdio::piped())" "$REPO_ROOT/src/cli/run.rs"; then
        log_pass "stdout is piped for port watching"
    else
        log_fail "stdout is piped for port watching"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# FEATURE 5: PACKAGE SCRIPT WRAPPING
# ═══════════════════════════════════════════════════════════════════════════════

test_wrap_script_help() {
    log_section "Feature #26: Package Script Wrapping"

    output=$($ANTRA_BIN add wrap-script --help 2>&1 || true)

    if echo "$output" | grep -q "Wrap a package.json script"; then
        log_pass "wrap-script has description"
    else
        log_fail "wrap-script has description"
    fi

    if echo "$output" | grep -q "\-\-command"; then
        log_pass "wrap-script has --command flag"
    else
        log_fail "wrap-script has --command flag"
    fi

    if echo "$output" | grep -q "\-\-port"; then
        log_pass "wrap-script has --port flag"
    else
        log_fail "wrap-script has --port flag"
    fi

    if echo "$output" | grep -q "\-\-force"; then
        log_pass "wrap-script has --force flag"
    else
        log_fail "wrap-script has --force flag"
    fi
}

test_wrap_script_no_package_json() {
    log_section "Feature #26: wrap-script without package.json"

    local dir="$TEST_DIR/no-package-json"
    mkdir -p "$dir"
    cd "$dir"

    output=$($ANTRA_BIN add wrap-script myapp --command "npm run dev" --port 3000 2>&1 || true)

    if echo "$output" | grep -q "No package.json found"; then
        log_pass "wrap-script shows error without package.json"
    else
        log_fail "wrap-script shows error without package.json"
    fi
}

test_wrap_script_creates_script() {
    log_section "Feature #26: wrap-script creates antra script"

    local dir="$TEST_DIR/wrap-test"
    mkdir -p "$dir"
    cd "$dir"

    cat > package.json << 'EOF'
{
  "name": "wrap-test-app",
  "scripts": {
    "dev": "node server.js"
  }
}
EOF

    output=$($ANTRA_BIN add wrap-script myapp --command "npm run dev" --port 3000 2>&1 || true)

    if echo "$output" | grep -q "Added script"; then
        log_pass "wrap-script adds script successfully"
    else
        log_fail "wrap-script adds script successfully"
    fi

    # Check the script was added
    if grep -q "antra:myapp" package.json; then
        log_pass "antra:myapp script added to package.json"
    else
        log_fail "antra:myapp script added to package.json"
    fi

    # Check the script content
    if grep -q "antra run --domain myapp.localhost --port 3000" package.json; then
        log_pass "Script contains correct antra run command"
    else
        log_fail "Script contains correct antra run command"
    fi
}

test_wrap_script_force_overwrite() {
    log_section "Feature #26: wrap-script --force overwrite"

    local dir="$TEST_DIR/wrap-force"
    mkdir -p "$dir"
    cd "$dir"

    cat > package.json << 'EOF'
{
  "name": "wrap-force-app",
  "scripts": {
    "dev": "node server.js"
  }
}
EOF

    # First run
    $ANTRA_BIN add wrap-script myapp --command "npm run dev" --port 3000 2>&1 || true

    # Second run without --force should fail
    output=$($ANTRA_BIN add wrap-script myapp --command "npm run dev" --port 3000 2>&1 || true)

    if echo "$output" | grep -q "already exists"; then
        log_pass "wrap-script rejects duplicate without --force"
    else
        log_fail "wrap-script rejects duplicate without --force"
    fi

    # With --force should succeed
    output=$($ANTRA_BIN add wrap-script myapp --command "npm run dev" --port 3001 --force 2>&1 || true)

    if echo "$output" | grep -q "Added script"; then
        log_pass "wrap-script --force overwrites existing"
    else
        log_fail "wrap-script --force overwrites existing"
    fi
}

test_wrap_script_package_json_format() {
    log_section "Feature #26: wrap-script preserves package.json format"

    local dir="$TEST_DIR/wrap-format"
    mkdir -p "$dir"
    cd "$dir"

    cat > package.json << 'EOF'
{
  "name": "format-app",
  "version": "1.0.0",
  "scripts": {
    "dev": "node server.js",
    "build": "webpack",
    "test": "jest"
  },
  "dependencies": {
    "express": "^4.18.0"
  }
}
EOF

    $ANTRA_BIN add wrap-script myapp --command "npm run dev" --port 3000 2>&1 || true

    # Check that existing scripts are preserved
    if grep -q '"build"' package.json; then
        log_pass "Existing scripts preserved"
    else
        log_fail "Existing scripts preserved"
    fi

    if grep -q '"test"' package.json; then
        log_pass "test script preserved"
    else
        log_fail "test script preserved"
    fi

    if grep -q '"express"' package.json; then
        log_pass "dependencies preserved"
    else
        log_fail "dependencies preserved"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# INTEGRATION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_all_new_commands_help() {
    log_section "Integration: All new commands in help"

    output=$($ANTRA_BIN --help 2>&1 || true)

    if echo "$output" | grep -q "add"; then
        log_pass "add command in main help"
    else
        log_fail "add command in main help"
    fi
}

test_roadmap_updated() {
    log_section "Integration: Roadmap updated"

    # ROADMAP.md, not roadmap.md: the repo file is upper-case, and macOS's
    # case-insensitive filesystem was the only reason the lower-case name
    # ever resolved. On a Linux CI runner it found nothing.
    local roadmap="$REPO_ROOT/ROADMAP.md"

    if grep -q "DONE" "$roadmap"; then
        log_pass "Roadmap has DONE status"
    else
        log_fail "Roadmap has DONE status"
    fi

    # Was `grep -q "Continuous Port Sync" roadmap.md | grep -q "DONE"`. The
    # first grep is `-q`, so it emits nothing and the second always fails —
    # and the `else` branch called `log_pass` anyway, making this assertion
    # incapable of failing. Match the feature and its DONE marker on the same
    # line, and fail properly when it is missing.
    if grep "Continuous Port Sync" "$roadmap" | grep -q "DONE"; then
        log_pass "Continuous Port Sync marked DONE"
    else
        log_fail "Continuous Port Sync marked DONE"
    fi

    if grep -q "Port Conflict Auto-Resolution" "$roadmap"; then
        log_pass "Port Conflict Auto-Resolution in roadmap"
    else
        log_fail "Port Conflict Auto-Resolution in roadmap"
    fi
}

test_portless_parity_complete() {
    log_section "Integration: Portless parity features"

    local features=("Continuous Port Sync" "Package Script Wrapping" "Smart Daemon Auto-Start" "Zero-Config" "Port Conflict Auto-Resolution")

    for feature in "${features[@]}"; do
        if grep -q "$feature" "$REPO_ROOT/ROADMAP.md"; then
            log_pass "Feature '$feature' documented"
        else
            log_fail "Feature '$feature' documented"
        fi
    done
}

# ═══════════════════════════════════════════════════════════════════════════════
# REAL END-TO-END TEST (with actual servers)
# ═══════════════════════════════════════════════════════════════════════════════

test_e2e_real_server() {
    log_section "E2E: Real server test through the proxy"

    local dir="$TEST_DIR/e2e-real"
    mkdir -p "$dir"
    cd "$dir"

    # A real HTTP server on a real port, fronted by a real Antra route, and
    # fetched back through the proxy over HTTPS. This is the one test in the
    # suite that exercises the whole path end to end.
    #
    # It used to be vacuous. It started the server with `timeout 5`, which is
    # GNU coreutils and absent on macOS, so the command failed instantly, the
    # `kill -0` check saw a dead pid, and the `else` branch reported a pass.
    # The test could not fail and never ran a server. No `timeout` here, and
    # every branch asserts something.
    local app_port
    app_port=$(free_port)

    node -e "require('http').createServer((q,r)=>r.end('Hello from test server!')).listen($app_port,'127.0.0.1')" &
    local server_pid=$!
    sleep 1

    if ! kill -0 "$server_pid" 2>/dev/null; then
        log_fail "Upstream server started on $app_port"
        return 0
    fi
    log_pass "Upstream server started on $app_port"

    output=$($ANTRA_BIN add route --domain e2e-test.localhost --port "$app_port" 2>&1 || true)
    if echo "$output" | grep -q "Route registered"; then
        log_pass "Route registered for the live server"
    else
        log_fail "Route registered for the live server"
        kill "$server_pid" 2>/dev/null || true
        return 0
    fi

    # Point at the port the daemon actually bound. $ANTRA_PORT is what the
    # suite runs with, because 443 needs root and 8443 is not reliably free.
    local https_port="${ANTRA_PORT:-443}"
    local body
    body=$(curl -sk --max-time 10 "https://e2e-test.localhost:${https_port}/" 2>/dev/null || true)

    if echo "$body" | grep -q "Hello from test server!"; then
        log_pass "Proxy forwards requests correctly over HTTPS"
    else
        log_fail "Proxy forwards requests correctly over HTTPS (got: ${body:0:80})"
    fi

    $ANTRA_BIN remove e2e-test.localhost >/dev/null 2>&1 || true
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
}

test_e2e_add_route() {
    log_section "E2E: antra add route with real server"

    local dir="$TEST_DIR/e2e-add"
    mkdir -p "$dir"
    cd "$dir"

    # Start a simple server on a probed-free port
    local app_port
    app_port=$(free_port)
    node -e "require('http').createServer((req,res)=>{res.end('add-test')}).listen($app_port,'127.0.0.1',()=>console.log('running'))" &
    local server_pid=$!

    sleep 1

    # Add route to it
    output=$($ANTRA_BIN add route --domain add-test.localhost --port "$app_port" 2>&1 || true)

    # Was `grep -q "Added route"`, for a line that no longer exists — a
    # previous session removed it as redundant (src/cli/add.rs). Worse, the
    # `else` branch also called log_pass, so the assertion could not fail
    # and was reporting a pass it had never earned.
    if echo "$output" | grep -q "Route registered"; then
        log_pass "add route registered successfully"
    else
        log_fail "add route registered successfully"
    fi

    # List routes
    list_output=$($ANTRA_BIN list 2>&1 || true)

    if echo "$list_output" | grep -q "add-test.localhost"; then
        log_pass "Route appears in list"
    else
        log_fail "Route appears in list"
    fi

    kill $server_pid 2>/dev/null || true
    wait $server_pid 2>/dev/null || true
}

# ═══════════════════════════════════════════════════════════════════════════════
# RUN ALL TESTS
# ═══════════════════════════════════════════════════════════════════════════════

main() {
    echo -e "${BOLD}${CYAN}Starting Antra Portless-Parity E2E Tests${RESET}"
    echo -e "${CYAN}$(date)${RESET}\n"

    # Build first
    echo -e "${YELLOW}Building antra...${RESET}"
    cargo build --quiet 2>&1 | grep -v "^warning" || true
    echo ""

    # Creates $TEST_DIR and the results header. Without it the scratch dirs
    # existed only as a side effect of the first test's `mkdir -p`, and every
    # result was `tee -a`'d onto a file that was never created.
    setup

    # Feature 1: Port Conflict Auto-Resolution
    test_port_conflict_help
    test_port_conflict_code
    test_port_conflict_used_in_run

    # Feature 2: Smart Daemon Auto-Start
    test_smart_daemon_code
    test_smart_daemon_in_list
    test_smart_daemon_in_alias
    test_smart_daemon_in_open
    test_smart_daemon_in_remove
    test_smart_daemon_in_prune
    test_smart_daemon_list_no_daemon

    # Feature 3: Zero-Config antra add
    test_add_command_help
    test_add_route_help
    test_add_route_no_daemon
    test_add_code_exists
    test_add_module_registered
    test_add_command_variant

    # Feature 4: Continuous Port Sync
    test_port_watcher_exists
    test_port_watcher_module
    test_port_watcher_function
    test_port_watcher_patterns
    test_port_watcher_used_in_run
    test_port_watcher_stdout_capture

    # Feature 5: Package Script Wrapping
    test_wrap_script_help
    test_wrap_script_no_package_json
    test_wrap_script_creates_script
    test_wrap_script_force_overwrite
    test_wrap_script_package_json_format

    # Integration tests
    test_all_new_commands_help
    test_roadmap_updated
    test_portless_parity_complete

    # Real E2E tests
    test_e2e_real_server
    test_e2e_add_route

    # Summary
    log_section "TEST SUMMARY"
    echo -e "${GREEN}Passed: $pass_count${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${RED}Failed: $fail_count${RESET}" | tee -a "$RESULTS_FILE"
    echo ""

    if [ "$fail_count" -eq 0 ]; then
        echo -e "${GREEN}${BOLD}ALL TESTS PASSED!${RESET}" | tee -a "$RESULTS_FILE"
    else
        echo -e "${RED}${BOLD}SOME TESTS FAILED${RESET}" | tee -a "$RESULTS_FILE"
    fi

    cleanup
    exit $fail_count
}

main "$@"
