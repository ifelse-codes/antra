#!/bin/bash
set -e

# Antra NEXT Sprint Features E2E Test Script
# Tests all newly implemented features

ANTRA_BIN="./target/debug/antra"
TEST_DIR="/tmp/antra-next-tests"
RESULTS_FILE="/tmp/antra-next-test-results.txt"

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
    echo -e "${GREEN}✓ PASS${RESET}: $1" | tee -a "$RESULTS_FILE"
    ((pass_count++))
}

log_fail() {
    echo -e "${RED}✗ FAIL${RESET}: $1" | tee -a "$RESULTS_FILE"
    ((fail_count++))
}

log_section() {
    echo -e "\n${BOLD}${CYAN}═══════════════════════════════════════════${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${BOLD}${CYAN}  $1${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${BOLD}${CYAN}═══════════════════════════════════════════${RESET}\n" | tee -a "$RESULTS_FILE"
}

cleanup() {
    rm -rf "$TEST_DIR"
    rm -f "$RESULTS_FILE"
}

setup() {
    cleanup
    mkdir -p "$TEST_DIR"
    echo "Antra NEXT Sprint E2E Test Results - $(date)" > "$RESULTS_FILE"
}

# ═══════════════════════════════════════════════════════════════════════════════
# PRUNE COMMAND TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_prune_no_daemon() {
    log_section "Task #9: antra prune (no daemon)"
    
    output=$($ANTRA_BIN prune 2>&1 || true)
    
    if echo "$output" | grep -q "ANTRA PRUNE"; then
        log_pass "Prune command shows header"
    else
        log_fail "Prune command shows header"
    fi
    
    if echo "$output" | grep -q "Daemon not running"; then
        log_pass "Prune detects daemon not running"
    else
        log_fail "Prune detects daemon not running"
    fi
    
    if echo "$output" | grep -q "Nothing to prune"; then
        log_pass "Prune shows correct message when no daemon"
    else
        log_fail "Prune shows correct message when no daemon"
    fi
}

test_prune_help() {
    log_section "Task #9: antra prune --help"
    
    output=$($ANTRA_BIN prune --help 2>&1 || true)
    
    if echo "$output" | grep -q "Kill orphaned dev servers"; then
        log_pass "Prune help shows description"
    else
        log_fail "Prune help shows description"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# FORCE FLAG TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_force_flag_in_help() {
    log_section "Task #10: --force flag in run --help"
    
    output=$($ANTRA_BIN run --help 2>&1 || true)
    
    if echo "$output" | grep -q "\-\-force"; then
        log_pass "Run command has --force flag"
    else
        log_fail "Run command has --force flag"
    fi
    
    if echo "$output" | grep -q "Kill existing process and take over the route"; then
        log_pass "--force flag has description"
    else
        log_fail "--force flag has description"
    fi
}

test_force_flag_in_dev_help() {
    log_section "Task #10: --force flag availability"
    
    output=$($ANTRA_BIN run --help 2>&1 || true)
    
    if echo "$output" | grep -q "\-\-force"; then
        log_pass "--force flag available in run command"
    else
        log_fail "--force flag available in run command"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# CUSTOM TLD TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_tld_flag_in_help() {
    log_section "Task #7: --tld flag in run --help"
    
    output=$($ANTRA_BIN run --help 2>&1 || true)
    
    if echo "$output" | grep -q "\-\-tld"; then
        log_pass "Run command has --tld flag"
    else
        log_fail "Run command has --tld flag"
    fi
    
    if echo "$output" | grep -q "Custom TLD"; then
        log_pass "--tld flag has description"
    else
        log_fail "--tld flag has description"
    fi
}

test_tld_domain_construction() {
    log_section "Task #7: TLD domain construction"
    
    # Create test project
    local dir="$TEST_DIR/tld-test"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "myapp",
  "scripts": {
    "dev": "node -e \"console.log('ANTRA_DOMAIN=' + process.env.ANTRA_DOMAIN)\""
  }
}
EOF
    
    output=$($ANTRA_BIN run --domain myapp --tld localhost --no-trust-prompt -- node -e "console.log('ANTRA_DOMAIN=' + process.env.ANTRA_DOMAIN)" 2>&1 || true)
    
    if echo "$output" | grep -q "ANTRA_DOMAIN=myapp.localhost"; then
        log_pass "TLD constructs domain correctly (myapp.localhost)"
    else
        log_fail "TLD constructs domain correctly (myapp.localhost)"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# HOSTS COMMAND TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_hosts_help() {
    log_section "Task #8: antra hosts --help"
    
    output=$($ANTRA_BIN hosts --help 2>&1 || true)
    
    if echo "$output" | grep -q "Manage /etc/hosts entries"; then
        log_pass "Hosts command has description"
    else
        log_fail "Hosts command has description"
    fi
    
    if echo "$output" | grep -q "sync"; then
        log_pass "Hosts has sync subcommand"
    else
        log_fail "Hosts has sync subcommand"
    fi
    
    if echo "$output" | grep -q "clean"; then
        log_pass "Hosts has clean subcommand"
    else
        log_fail "Hosts has clean subcommand"
    fi
}

test_hosts_sync_help() {
    log_section "Task #8: antra hosts sync --help"
    
    output=$($ANTRA_BIN hosts sync --help 2>&1 || true)
    
    if echo "$output" | grep -q "Sync .localhost domains"; then
        log_pass "Hosts sync has description"
    else
        log_fail "Hosts sync has description"
    fi
}

test_hosts_clean_help() {
    log_section "Task #8: antra hosts clean --help"
    
    output=$($ANTRA_BIN hosts clean --help 2>&1 || true)
    
    if echo "$output" | grep -q "Remove all Antra-managed"; then
        log_pass "Hosts clean has description"
    else
        log_fail "Hosts clean has description"
    fi
}

test_hosts_sync_no_daemon() {
    log_section "Task #8: antra hosts sync (no daemon)"
    
    output=$($ANTRA_BIN hosts sync 2>&1 || true)
    
    if echo "$output" | grep -q "ANTRA HOSTS SYNC"; then
        log_pass "Hosts sync shows header"
    else
        log_fail "Hosts sync shows header"
    fi
    
    if echo "$output" | grep -q "Daemon not running"; then
        log_pass "Hosts sync detects daemon not running"
    else
        log_fail "Hosts sync detects daemon not running"
    fi
}

test_hosts_clean_no_entries() {
    log_section "Task #8: antra hosts clean (no entries)"
    
    output=$($ANTRA_BIN hosts clean 2>&1 || true)
    
    if echo "$output" | grep -q "ANTRA HOSTS CLEAN"; then
        log_pass "Hosts clean shows header"
    else
        log_fail "Hosts clean shows header"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# SERVICE COMMAND TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_service_help() {
    log_section "Task #6: antra service --help"
    
    output=$($ANTRA_BIN service --help 2>&1 || true)
    
    if echo "$output" | grep -q "Manage Antra as a system service"; then
        log_pass "Service command has description"
    else
        log_fail "Service command has description"
    fi
    
    if echo "$output" | grep -q "install"; then
        log_pass "Service has install subcommand"
    else
        log_fail "Service has install subcommand"
    fi
    
    if echo "$output" | grep -q "status"; then
        log_pass "Service has status subcommand"
    else
        log_fail "Service has status subcommand"
    fi
    
    if echo "$output" | grep -q "uninstall"; then
        log_pass "Service has uninstall subcommand"
    else
        log_fail "Service has uninstall subcommand"
    fi
}

test_service_status() {
    log_section "Task #6: antra service status"
    
    output=$($ANTRA_BIN service status 2>&1 || true)
    
    if echo "$output" | grep -q "ANTRA SERVICE STATUS"; then
        log_pass "Service status shows header"
    else
        log_fail "Service status shows header"
    fi
    
    if echo "$output" | grep -q "not installed"; then
        log_pass "Service status shows not installed"
    else
        log_fail "Service status shows not installed"
    fi
}

test_service_install_help() {
    log_section "Task #6: antra service install --help"
    
    output=$($ANTRA_BIN service install --help 2>&1 || true)
    
    if echo "$output" | grep -q "Install Antra as a system service"; then
        log_pass "Service install has description"
    else
        log_fail "Service install has description"
    fi
}

test_service_uninstall_help() {
    log_section "Task #6: antra service uninstall --help"
    
    output=$($ANTRA_BIN service uninstall --help 2>&1 || true)
    
    if echo "$output" | grep -q "Uninstall Antra service"; then
        log_pass "Service uninstall has description"
    else
        log_fail "Service uninstall has description"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# LOOP DETECTION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_loop_detection_code() {
    log_section "Task #11: Loop detection implementation"
    
    # Check that loop detection is implemented in http.rs
    if grep -q "508 Loop Detected" src/proxy/http.rs; then
        log_pass "Loop detection returns 508 status"
    else
        log_fail "Loop detection returns 508 status"
    fi
    
    if grep -q "x-antra-hops" src/proxy/http.rs; then
        log_pass "Loop detection checks x-antra-hops header"
    else
        log_fail "Loop detection checks x-antra-hops header"
    fi
    
    if grep -q "MAX_HOPS" src/proxy/http.rs; then
        log_pass "Loop detection has MAX_HOPS constant"
    else
        log_fail "Loop detection has MAX_HOPS constant"
    fi
    
    # Check WebSocket loop detection
    if grep -q "LOOP_DETECTED" src/proxy/websocket.rs; then
        log_pass "WebSocket loop detection implemented"
    else
        log_fail "WebSocket loop detection implemented"
    fi
    
    # Check hop count increment in forward.rs
    if grep -q "x-antra-hops" src/proxy/forward.rs; then
        log_pass "Hop count incremented in forward.rs"
    else
        log_fail "Hop count incremented in forward.rs"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# INTEGRATION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_all_commands_available() {
    log_section "Integration: All commands available"
    
    output=$($ANTRA_BIN --help 2>&1 || true)
    
    local commands=("run" "dev" "list" "doctor" "trust" "proxy" "clean" "alias" "open" "remove" "prune" "hosts" "service")
    
    for cmd in "${commands[@]}"; do
        if echo "$output" | grep -q "$cmd"; then
            log_pass "Command '$cmd' available"
        else
            log_fail "Command '$cmd' available"
        fi
    done
}

test_run_command_options() {
    log_section "Integration: run command options"
    
    output=$($ANTRA_BIN run --help 2>&1 || true)
    
    local options=("--domain" "--port" "--tld" "--allow-custom-domain" "--no-trust-prompt" "--yes" "--force")
    
    for opt in "${options[@]}"; do
        if echo "$output" | grep -q "$opt"; then
            log_pass "Option '$opt' available in run"
        else
            log_fail "Option '$opt' available in run"
        fi
    done
}

test_env_injection_with_tld() {
    log_section "Integration: Env injection with TLD"
    
    local dir="$TEST_DIR/env-tld-test"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "envtest",
  "scripts": {
    "dev": "node -e \"console.log('PORT=' + process.env.PORT + ' HOST=' + process.env.HOST + ' ANTRA_DOMAIN=' + process.env.ANTRA_DOMAIN + ' ANTRA_URL=' + process.env.ANTRA_URL)\""
  }
}
EOF
    
    output=$($ANTRA_BIN run --domain envtest --tld localhost --no-trust-prompt -- node -e "console.log('PORT=' + process.env.PORT + ' HOST=' + process.env.HOST + ' ANTRA_DOMAIN=' + process.env.ANTRA_DOMAIN + ' ANTRA_URL=' + process.env.ANTRA_URL)" 2>&1 || true)
    
    if echo "$output" | grep -q "PORT=[0-9]"; then
        log_pass "PORT env var injected with TLD"
    else
        log_fail "PORT env var injected with TLD"
    fi
    
    if echo "$output" | grep -q "HOST=127.0.0.1"; then
        log_pass "HOST env var injected with TLD"
    else
        log_fail "HOST env var injected with TLD"
    fi
    
    if echo "$output" | grep -q "ANTRA_DOMAIN=envtest.localhost"; then
        log_pass "ANTRA_DOMAIN env var correct with TLD"
    else
        log_fail "ANTRA_DOMAIN env var correct with TLD"
    fi
    
    if echo "$output" | grep -q "ANTRA_URL=https://envtest.localhost"; then
        log_pass "ANTRA_URL env var correct with TLD"
    else
        log_fail "ANTRA_URL env var correct with TLD"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# RUN ALL TESTS
# ═══════════════════════════════════════════════════════════════════════════════

main() {
    echo -e "${BOLD}${CYAN}Starting Antra NEXT Sprint E2E Tests${RESET}"
    echo -e "${CYAN}$(date)${RESET}\n"
    
    # Build first
    echo -e "${YELLOW}Building antra...${RESET}"
    cargo build --quiet 2>&1 | grep -v "^warning" || true
    echo ""
    
    # Task #9: Prune tests
    test_prune_no_daemon
    test_prune_help
    
    # Task #10: Force flag tests
    test_force_flag_in_help
    test_force_flag_in_dev_help
    
    # Task #7: Custom TLD tests
    test_tld_flag_in_help
    test_tld_domain_construction
    
    # Task #8: Hosts command tests
    test_hosts_help
    test_hosts_sync_help
    test_hosts_clean_help
    test_hosts_sync_no_daemon
    test_hosts_clean_no_entries
    
    # Task #6: Service command tests
    test_service_help
    test_service_status
    test_service_install_help
    test_service_uninstall_help
    
    # Task #11: Loop detection tests
    test_loop_detection_code
    
    # Integration tests
    test_all_commands_available
    test_run_command_options
    test_env_injection_with_tld
    
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
