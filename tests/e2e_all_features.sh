#!/bin/bash
set -e

# Antra Comprehensive E2E Test Script
# Tests all supported languages/frameworks and features

# Every test below `cd`s into a scratch directory under $TEST_DIR, so
# anything resolved against the working directory breaks after the first
# one. Both paths are anchored to the repo instead: $ANTRA_BIN to reach the
# binary, $REPO_ROOT for the source-level assertions near the end.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANTRA_BIN="${ANTRA_BIN:-$REPO_ROOT/target/debug/antra}"

# Per-worktree tag, so parallel suite runs cannot collide. The scratch
# directory and results file below used to be fixed paths, and setup() opens
# with `rm -rf "$TEST_DIR"` — so two worktrees running the suites at the same
# time deleted each other's scratch tree mid-run and overwrote each other's
# results. Git worktrees isolate files; they do not isolate /tmp.
WT_TAG="$(basename "$(pwd)" | tr -c 'A-Za-z0-9._-' '-')"
TEST_DIR="/tmp/antra-e2e-tests-$WT_TAG"
RESULTS_FILE="/tmp/antra-test-results-$WT_TAG.txt"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

pass_count=0
fail_count=0
skip_count=0

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

# A missing interpreter is not a broken feature. Reporting FAIL for
# "yarn is not installed" makes the suite red on any machine without the
# full toolchain, which trains people to ignore it. SKIP is counted and
# printed in the summary, and only a real assertion failure turns the suite
# red.
log_skip() {
    echo -e "${YELLOW}⊘ SKIP${RESET}: $1" | tee -a "$RESULTS_FILE"
    skip_count=$((skip_count + 1))
}

# True when every named command is on PATH. Used to gate whole test
# functions, so a skipped test reports once instead of once per assertion.
have() {
    local c
    for c in "$@"; do
        command -v "$c" >/dev/null 2>&1 || return 1
    done
    return 0
}

# Gate a test function on its toolchain:
#   need "yarn" "Node.js (yarn)" || return 0
need() {
    local cmds="$1" label="$2"
    if have $cmds; then
        return 0
    fi
    log_skip "$label — needs: $cmds"
    return 1
}

# Run an `antra` invocation with a wall-clock ceiling.
#
# `antra dev` stays in the foreground running the project's own dev command.
# When that command is a server — `python -m http.server`, a web `cargo run`,
# `go run` — it never returns, and the suite hangs until the CI job's own
# timeout. Observed on ubuntu-latest: 45 minutes, because `python` is on the
# runner's PATH there so the test runs instead of skipping, while on a Mac
# `python` is absent and the test skips. The hang was invisible locally for
# exactly that reason.
#
# `timeout(1)` is GNU coreutils and absent on macOS, so this is a background
# job plus a kill — both of which every POSIX shell has. Output is still
# captured, so callers keep writing `output=$(run_antra_capped ...)`.
#
# The kill has to be surgical. Antra's children are two: the long-lived daemon
# and the project's dev command. They are told apart by process group — the
# dev command is put in its own group (see docs/security.md, "Process Safety")
# and therefore leads it, while the daemon shares antra's. Killing the group
# of a child whose pgid equals its own pid takes the server down and leaves the
# daemon alone. Killing the group blindly would take the daemon with it and
# force the next test to rebind its ports.
ANTRA_TIMEOUT="${ANTRA_TIMEOUT:-25}"

run_antra_capped() {
    local pid killer rc
    "$@" &
    pid=$!
    (
        sleep "$ANTRA_TIMEOUT"
        local child pgid
        for child in $(pgrep -P "$pid" 2>/dev/null); do
            pgid=$(ps -o pgid= -p "$child" 2>/dev/null | tr -d ' ')
            [ -n "$pgid" ] && [ "$pgid" = "$child" ] || continue
            kill -TERM -"$child" 2>/dev/null
        done
        kill -TERM "$pid" 2>/dev/null
    ) &
    killer=$!
    wait "$pid" 2>/dev/null
    rc=$?
    kill -TERM "$killer" 2>/dev/null
    wait "$killer" 2>/dev/null
    return $rc
}

# An assertion that needs a specific binary on PATH.
#
# `antra dev` prints `Started: <cmd>` only after a successful spawn, so on a
# lean CI runner without that toolchain the line never appears and the check
# fails for a reason that has nothing to do with Antra. SKIP and count it
# instead. A missing interpreter is not a broken feature.
#
# This gates the individual assertion rather than the whole test, because the
# detection assertion above it needs nothing installed and is still worth
# running.
assert_needs_bin() {
    local bin="$1" pattern="$2" label="$3"
    if ! have "$bin"; then
        log_skip "$label — needs: $bin"
        return 0
    fi
    if echo "$output" | grep -q "$pattern"; then
        log_pass "$label"
    else
        log_fail "$label"
    fi
}

# True when a Ruby gem is installed. `bundle exec rails server` needs the gem,
# not a `rails` binary — Rails ships no global executable — so a PATH check
# would test the wrong thing.
have_gem() { gem list -i "$1" >/dev/null 2>&1; }

# Echo a port nothing is listening on.
#
# The framework tests want to observe two different things: which default
# port the detector picks, and which command it runs. Picking the default
# only works when that port is free — and 8080 frequently is not, on a
# developer's machine or a shared CI runner. So the port choice is asserted
# from a bare run (where the chosen port is named either way, whether the
# bind then succeeds or is rejected as in use), and the command is asserted
# from a second run forced onto a port this picks.
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

log_section() {
    echo -e "\n${BOLD}${CYAN}═══════════════════════════════════════════${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${BOLD}${CYAN}  $1${RESET}" | tee -a "$RESULTS_FILE"
    echo -e "${BOLD}${CYAN}═══════════════════════════════════════════${RESET}\n" | tee -a "$RESULTS_FILE"
}

cleanup() {
    rm -rf "$TEST_DIR"
    # $RESULTS_FILE is the record of this run — it is the artifact the caller
    # reads. Only the previous run's file is cleared, by setup().
}

setup() {
    rm -rf "$TEST_DIR"
    mkdir -p "$TEST_DIR"
    echo "Antra E2E Test Results - $(date)" > "$RESULTS_FILE"
}

# ═══════════════════════════════════════════════════════════════════════════════
# NODE.JS TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_node_npm() {
    log_section "Node.js (npm)"
    local dir="$TEST_DIR/node-npm"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-npm-app",
  "version": "1.0.0",
  "scripts": {
    "dev": "node -e \"console.log('PORT=' + process.env.PORT + ' HOST=' + process.env.HOST + ' ANTRA_DOMAIN=' + process.env.ANTRA_DOMAIN + ' ANTRA_URL=' + process.env.ANTRA_URL + ' NODE_EXTRA_CA_CERTS=' + process.env.NODE_EXTRA_CA_CERTS)\""
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Node.js project: test-npm-app"; then
        log_pass "Node.js detection from package.json"
    else
        log_fail "Node.js detection from package.json"
    fi
    
    if echo "$output" | grep -q "npm run dev"; then
        log_pass "npm command inferred correctly"
    else
        log_fail "npm command inferred correctly"
    fi
    
    if echo "$output" | grep -q "PORT="; then
        log_pass "PORT env var injected"
    else
        log_fail "PORT env var injected"
    fi
    
    if echo "$output" | grep -q "HOST=127.0.0.1"; then
        log_pass "HOST env var injected"
    else
        log_fail "HOST env var injected"
    fi
    
    if echo "$output" | grep -q "ANTRA_DOMAIN=test-npm-app.localhost"; then
        log_pass "ANTRA_DOMAIN env var injected"
    else
        log_fail "ANTRA_DOMAIN env var injected"
    fi
    
    if echo "$output" | grep -q "ANTRA_URL=https://test-npm-app.localhost"; then
        log_pass "ANTRA_URL env var injected"
    else
        log_fail "ANTRA_URL env var injected"
    fi
}

test_node_yarn() {
    log_section "Node.js (yarn)"
    need "yarn" "Node.js (yarn)" || return 0
    local dir="$TEST_DIR/node-yarn"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-yarn-app",
  "scripts": {
    "dev": "node -e \"console.log('PORT=' + process.env.PORT)\""
  }
}
EOF
    touch yarn.lock
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "yarn dev"; then
        log_pass "yarn command inferred correctly"
    else
        log_fail "yarn command inferred correctly"
    fi
}

test_node_pnpm() {
    log_section "Node.js (pnpm)"
    local dir="$TEST_DIR/node-pnpm"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-pnpm-app",
  "scripts": {
    "dev": "node -e \"console.log('PORT=' + process.env.PORT)\""
  }
}
EOF
    touch pnpm-lock.yaml
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    # ROADMAP C13, answered. The lockfile picks pnpm whether or not pnpm is
    # installed: with it stripped from PATH the spawn fails *naming pnpm*. The
    # old check grepped the "Started: pnpm run dev" line, which only exists
    # after a successful spawn, so it failed on every runner without pnpm —
    # the C12 rule again. Assert the inference either way instead of skipping:
    # the spawn line where pnpm exists, the spawn error naming it where not.
    # Either can fail: without the lockfile this infers npm, and neither
    # string appears.
    local want="Failed to spawn 'pnpm'"
    have pnpm && want="pnpm run dev"
    if echo "$output" | grep -q "$want"; then
        log_pass "pnpm command inferred correctly"
    else
        log_fail "pnpm command inferred correctly"
    fi
}

test_node_bun() {
    log_section "Node.js (bun)"
    need "bun" "Node.js (bun)" || return 0
    local dir="$TEST_DIR/node-bun"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-bun-app",
  "scripts": {
    "dev": "node -e \"console.log('PORT=' + process.env.PORT)\""
  }
}
EOF
    touch bun.lockb
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "bun run dev"; then
        log_pass "bun command inferred correctly"
    else
        log_fail "bun command inferred correctly"
    fi
}

test_vite() {
    log_section "Node.js (Vite)"
    local dir="$TEST_DIR/node-vite"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-vite-app",
  "devDependencies": {
    "vite": "^5.0.0"
  },
  "scripts": {
    "dev": "vite"
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Node.js project: test-vite-app"; then
        log_pass "Vite project detected"
    else
        log_fail "Vite project detected"
    fi
}

test_nextjs() {
    log_section "Node.js (Next.js)"
    local dir="$TEST_DIR/node-next"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-next-app",
  "dependencies": {
    "next": "^14.0.0"
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Node.js project: test-next-app"; then
        log_pass "Next.js project detected"
    else
        log_fail "Next.js project detected"
    fi
}

test_nuxt() {
    log_section "Node.js (Nuxt)"
    local dir="$TEST_DIR/node-nuxt"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-nuxt-app",
  "dependencies": {
    "nuxt": "^3.0.0"
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Node.js project: test-nuxt-app"; then
        log_pass "Nuxt project detected"
    else
        log_fail "Nuxt project detected"
    fi
}

test_react_cra() {
    log_section "Node.js (Create React App)"
    local dir="$TEST_DIR/node-cra"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-cra-app",
  "dependencies": {
    "react-scripts": "^5.0.0"
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Node.js project: test-cra-app"; then
        log_pass "React CRA project detected"
    else
        log_fail "React CRA project detected"
    fi
}

test_angular() {
    log_section "Node.js (Angular)"
    local dir="$TEST_DIR/node-angular"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-angular-app",
  "devDependencies": {
    "@angular/cli": "^17.0.0"
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Node.js project: test-angular-app"; then
        log_pass "Angular project detected"
    else
        log_fail "Angular project detected"
    fi
}

test_node_no_scripts() {
    log_section "Node.js (no scripts, fallback)"
    local dir="$TEST_DIR/node-noscripts"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "test-noscripts-app"
}
EOF
    mkdir -p node_modules
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "npm start"; then
        log_pass "npm start fallback works"
    else
        log_fail "npm start fallback works"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# RUST TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_rust_axum() {
    log_section "Rust (axum)"
    local dir="$TEST_DIR/rust-axum"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Cargo.toml << 'EOF'
[package]
name = "test-axum-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Rust project: test-axum-app"; then
        log_pass "Rust project detected"
    else
        log_fail "Rust project detected"
    fi
    
    # `detect.rs` gives a Rust web framework 8080. Assert the *choice*, not
    # the successful bind: when 8080 is occupied the run stops with a
    # port-already-in-use error, which still proves 8080 was the port
    # selected. Asserting on 127.0.0.1:8080 would only pass on a machine
    # where nothing else happens to hold it.
    if echo "$output" | grep -q "8080"; then
        log_pass "Default port 8080 for axum"
    else
        log_fail "Default port 8080 for axum"
    fi
    
    # The command is only printed once the run proceeds, so force a port
    # this machine is not using.
    free=$(free_port)
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt --port "$free" 2>&1 || true)
    
    if echo "$output" | grep -q "cargo run"; then
        log_pass "cargo run command used"
    else
        log_fail "cargo run command used"
    fi
}

test_rust_actix() {
    log_section "Rust (actix-web)"
    local dir="$TEST_DIR/rust-actix"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Cargo.toml << 'EOF'
[package]
name = "test-actix-app"
version = "0.1.0"
edition = "2021"

[dependencies]
actix-web = "4"
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Rust project: test-actix-app"; then
        log_pass "Rust actix project detected"
    else
        log_fail "Rust actix project detected"
    fi
}

test_rust_rocket() {
    log_section "Rust (rocket)"
    local dir="$TEST_DIR/rust-rocket"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Cargo.toml << 'EOF'
[package]
name = "test-rocket-app"
version = "0.1.0"
edition = "2021"

[dependencies]
rocket = "0.5"
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Rust project: test-rocket-app"; then
        log_pass "Rust rocket project detected"
    else
        log_fail "Rust rocket project detected"
    fi
}

test_rust_no_web() {
    log_section "Rust (no web framework)"
    local dir="$TEST_DIR/rust-noweb"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Cargo.toml << 'EOF'
[package]
name = "test-noweb-app"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1"
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Rust project: test-noweb-app"; then
        log_pass "Rust non-web project detected"
    else
        log_fail "Rust non-web project detected"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# GO TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_go_gin() {
    log_section "Go (gin)"
    local dir="$TEST_DIR/go-gin"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > go.mod << 'EOF'
module test-gin-app

go 1.21

require github.com/gin-gonic/gin v1.9.1
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Go project: test-gin-app"; then
        log_pass "Go project detected"
    else
        log_fail "Go project detected"
    fi
    
    # Default port choice, asserted as the choice and not as a successful
    # bind — see the axum test for why.
    if echo "$output" | grep -q "8080"; then
        log_pass "Default port 8080 for Go"
    else
        log_fail "Default port 8080 for Go"
    fi
    
    # The command is only printed once the run proceeds, so force a port
    # this machine is not using.
    free=$(free_port)
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt --port "$free" 2>&1 || true)
    
    assert_needs_bin go "go run" "go run command used"
}

test_go_echo() {
    log_section "Go (echo)"
    local dir="$TEST_DIR/go-echo"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > go.mod << 'EOF'
module test-echo-app

go 1.21

require github.com/labstack/echo v4.6.3
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Go project: test-echo-app"; then
        log_pass "Go echo project detected"
    else
        log_fail "Go echo project detected"
    fi
}

test_go_fiber() {
    log_section "Go (fiber)"
    local dir="$TEST_DIR/go-fiber"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > go.mod << 'EOF'
module test-fiber-app

go 1.21

require github.com/gofiber/fiber/v2 v2.52.0
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Go project: test-fiber-app"; then
        log_pass "Go fiber project detected"
    else
        log_fail "Go fiber project detected"
    fi
}

test_go_chi() {
    log_section "Go (chi)"
    local dir="$TEST_DIR/go-chi"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > go.mod << 'EOF'
module test-chi-app

go 1.21

require github.com/go-chi/chi/v5 v5.0.12
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Go project: test-chi-app"; then
        log_pass "Go chi project detected"
    else
        log_fail "Go chi project detected"
    fi
}

test_go_no_web() {
    log_section "Go (no web framework)"
    local dir="$TEST_DIR/go-noweb"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > go.mod << 'EOF'
module test-noweb-app

go 1.21
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Go project: test-noweb-app"; then
        log_pass "Go non-web project detected"
    else
        log_fail "Go non-web project detected"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# PYTHON TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_python_fastapi() {
    log_section "Python (FastAPI)"
    need "python" "Python (FastAPI)" || return 0
    local dir="$TEST_DIR/python-fastapi"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > pyproject.toml << 'EOF'
[project]
name = "test-fastapi-app"
version = "0.1.0"
dependencies = [
    "fastapi>=0.100.0",
    "uvicorn>=0.23.0"
]
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Python project: test-fastapi-app"; then
        log_pass "Python project detected"
    else
        log_fail "Python project detected"
    fi
    
    # `uvicorn` has to be present for this to mean anything. Without it the
    # run stops at "Failed to spawn 'uvicorn'" — and grepping for "uvicorn"
    # then matches that error message, so the assertion passes while proving
    # nothing. The chosen command is only printed after a successful spawn.
    if ! have uvicorn; then
        log_skip "uvicorn command used for FastAPI — needs: uvicorn"
    elif echo "$output" | grep -q "uvicorn"; then
        log_pass "uvicorn command used for FastAPI"
    else
        log_fail "uvicorn command used for FastAPI"
    fi
    
    # Assert the *choice* of port, not a registered route. `uvicorn` is not
    # installed on every machine, and without it the run stops at
    # "Failed to spawn 'uvicorn'" before a route is ever registered — so
    # requiring `127.0.0.1:8000` tested whether uvicorn was installed, not
    # whether the port was chosen. The choice is printed either way, exactly as
    # the axum and Go tests rely on.
    if echo "$output" | grep -q "8000"; then
        log_pass "Default port 8000 for FastAPI"
    else
        log_fail "Default port 8000 for FastAPI"
    fi
}

test_python_django() {
    log_section "Python (Django)"
    need "python" "Python (Django)" || return 0
    local dir="$TEST_DIR/python-django"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > pyproject.toml << 'EOF'
[project]
name = "test-django-app"
version = "0.1.0"
dependencies = [
    "django>=4.2"
]
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Python project: test-django-app"; then
        log_pass "Python Django project detected"
    else
        log_fail "Python Django project detected"
    fi
    
    if echo "$output" | grep -q "manage.py runserver"; then
        log_pass "Django manage.py runserver used"
    else
        log_fail "Django manage.py runserver used"
    fi
}

test_python_flask() {
    log_section "Python (Flask)"
    need "python" "Python (Flask)" || return 0
    local dir="$TEST_DIR/python-flask"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > pyproject.toml << 'EOF'
[project]
name = "test-flask-app"
version = "0.1.0"
dependencies = [
    "flask>=3.0.0"
]
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Python project: test-flask-app"; then
        log_pass "Python Flask project detected"
    else
        log_fail "Python Flask project detected"
    fi
    
    # Flask's default is 5000, and macOS Control Center holds 5000
    # permanently (`ControlCe` listening on *:commplex-main). This run can
    # therefore never get as far as printing the command it chose. Assert the
    # port choice here — the occupied-port error still names 5000 — and the
    # command from a second run forced onto a probed-free port, the same split
    # the axum and Go tests use.
    if echo "$output" | grep -q "5000"; then
        log_pass "Default port 5000 for Flask"
    else
        log_fail "Default port 5000 for Flask"
    fi

    free=$(free_port)
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt --port "$free" 2>&1 || true)

    # The command string is only ever printed once the spawn is attempted, so
    # asserting it needs `flask` present. Without it the run stops at
    # "Failed to spawn 'flask'", which names the binary but not the argument
    # list — grepping for "flask run" would then be asserting nothing.
    if ! have flask; then
        log_skip "Flask command used — needs: flask"
    elif echo "$output" | grep -q "flask run"; then
        log_pass "Flask command used"
    else
        log_fail "Flask command used"
    fi
}

test_python_generic() {
    log_section "Python (generic)"
    need "python" "Python (generic)" || return 0
    local dir="$TEST_DIR/python-generic"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > pyproject.toml << 'EOF'
[project]
name = "test-generic-app"
version = "0.1.0"
dependencies = [
    "requests>=2.31.0"
]
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Python project: test-generic-app"; then
        log_pass "Python generic project detected"
    else
        log_fail "Python generic project detected"
    fi
    
    if echo "$output" | grep -q "python -m http.server"; then
        log_pass "Python http.server fallback used"
    else
        log_fail "Python http.server fallback used"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# RUBY TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_ruby_rails() {
    log_section "Ruby (Rails)"
    local dir="$TEST_DIR/ruby-rails"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Gemfile << 'EOF'
source 'https://rubygems.org'

gem 'rails', '~> 7.0'
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Ruby on Rails project"; then
        log_pass "Ruby Rails project detected"
    else
        log_fail "Ruby Rails project detected"
    fi
    
    # The command is only printed after a successful spawn, and spawning needs
    # the rails *gem* — the Gemfile fixture declares it but nothing installs it
    # on a lean runner. Rails has no global `rails` binary, so a PATH check
    # would be the wrong test.
    if ! have_gem rails; then
        log_skip "Rails server command used — needs: the rails gem"
    elif echo "$output" | grep -q "bundle exec rails server"; then
        log_pass "Rails server command used"
    else
        log_fail "Rails server command used"
    fi

    # The port *choice*, not a registered route: without the gem the run stops
    # at the spawn, so `127.0.0.1:3000` would only ever appear where the gems
    # happen to be installed. The chosen port is printed either way, exactly as
    # the axum and Go tests rely on.
    if echo "$output" | grep -q "3000"; then
        log_pass "Default port 3000 for Rails"
    else
        log_fail "Default port 3000 for Rails"
    fi
}

test_ruby_sinatra() {
    log_section "Ruby (Sinatra)"
    local dir="$TEST_DIR/ruby-sinatra"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Gemfile << 'EOF'
source 'https://rubygems.org'

gem 'sinatra'
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Ruby (Sinatra) project"; then
        log_pass "Ruby Sinatra project detected"
    else
        log_fail "Ruby Sinatra project detected"
    fi
    
    # Port *choice*, not a registered route — without the sinatra gem the run
    # stops at the spawn, so `127.0.0.1:4567` would test the gem, not Antra.
    if echo "$output" | grep -q "4567"; then
        log_pass "Default port 4567 for Sinatra"
    else
        log_fail "Default port 4567 for Sinatra"
    fi
}

test_ruby_generic() {
    log_section "Ruby (generic)"
    local dir="$TEST_DIR/ruby-generic"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > Gemfile << 'EOF'
source 'https://rubygems.org'

gem 'rake'
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Ruby project"; then
        log_pass "Ruby generic project detected"
    else
        log_fail "Ruby generic project detected"
    fi
    
    assert_needs_bin rackup "bundle exec rackup" "Rackup command used"
}

# ═══════════════════════════════════════════════════════════════════════════════
# ELIXIR TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_elixir_phoenix() {
    log_section "Elixir (Phoenix)"
    need "mix" "Elixir (Phoenix)" || return 0
    local dir="$TEST_DIR/elixir-phoenix"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > mix.exs << 'EOF'
defmodule TestPhoenixApp.MixProject do
  use Mix.Project

  def project do
    [
      app: :test_phoenix_app,
      version: "0.1.0",
      elixir: "~> 1.14",
      deps: [
        {:phoenix, "~> 1.7.0"}
      ]
    ]
  end
end
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Elixir (Phoenix) project"; then
        log_pass "Elixir Phoenix project detected"
    else
        log_fail "Elixir Phoenix project detected"
    fi
    
    if echo "$output" | grep -q "mix phx.server"; then
        log_pass "Phoenix server command used"
    else
        log_fail "Phoenix server command used"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:4000"; then
        log_pass "Default port 4000 for Phoenix"
    else
        log_fail "Default port 4000 for Phoenix"
    fi
}

test_elixir_generic() {
    log_section "Elixir (generic)"
    need "mix" "Elixir (generic)" || return 0
    local dir="$TEST_DIR/elixir-generic"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > mix.exs << 'EOF'
defmodule TestGenericApp.MixProject do
  use Mix.Project

  def project do
    [
      app: :test_generic_app,
      version: "0.1.0",
      elixir: "~> 1.14"
    ]
  end
end
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Elixir project"; then
        log_pass "Elixir generic project detected"
    else
        log_fail "Elixir generic project detected"
    fi
    
    if echo "$output" | grep -q "mix run"; then
        log_pass "mix run command used"
    else
        log_fail "mix run command used"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# PHP TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_php_laravel() {
    log_section "PHP (Laravel)"
    need "php" "PHP (Laravel)" || return 0
    local dir="$TEST_DIR/php-laravel"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > composer.json << 'EOF'
{
    "name": "test/laravel-app",
    "require": {
        "php": "^8.1",
        "laravel/framework": "^10.0"
    }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected PHP (Laravel) project"; then
        log_pass "PHP Laravel project detected"
    else
        log_fail "PHP Laravel project detected"
    fi
    
    if echo "$output" | grep -q "php artisan serve"; then
        log_pass "Laravel artisan serve used"
    else
        log_fail "Laravel artisan serve used"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:8000"; then
        log_pass "Default port 8000 for Laravel"
    else
        log_fail "Default port 8000 for Laravel"
    fi
}

test_php_generic() {
    log_section "PHP (generic)"
    need "php" "PHP (generic)" || return 0
    local dir="$TEST_DIR/php-generic"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > composer.json << 'EOF'
{
    "name": "test/generic-app",
    "require": {
        "php": "^8.1"
    }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected PHP project"; then
        log_pass "PHP generic project detected"
    else
        log_fail "PHP generic project detected"
    fi
    
    if echo "$output" | grep -q "php -S"; then
        log_pass "PHP built-in server used"
    else
        log_fail "PHP built-in server used"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# ENVIRONMENT INJECTION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_env_injection() {
    log_section "Environment Variable Injection"
    local dir="$TEST_DIR/env-test"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "env-test-app",
  "scripts": {
    "dev": "node -e \"const vars = ['PORT','HOST','ANTRA_DOMAIN','ANTRA_URL','NODE_EXTRA_CA_CERTS']; vars.forEach(v => console.log(v + '=' + (process.env[v] || 'NOT_SET')));\""
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "PORT=[0-9]"; then
        log_pass "PORT environment variable set"
    else
        log_fail "PORT environment variable set"
    fi
    
    if echo "$output" | grep -q "HOST=127.0.0.1"; then
        log_pass "HOST environment variable set to 127.0.0.1"
    else
        log_fail "HOST environment variable set to 127.0.0.1"
    fi
    
    if echo "$output" | grep -q "ANTRA_DOMAIN=env-test-app.localhost"; then
        log_pass "ANTRA_DOMAIN environment variable set"
    else
        log_fail "ANTRA_DOMAIN environment variable set"
    fi
    
    if echo "$output" | grep -q "ANTRA_URL=https://env-test-app.localhost"; then
        log_pass "ANTRA_URL environment variable set"
    else
        log_fail "ANTRA_URL environment variable set"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# PORT DETECTION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_port_detection() {
    log_section "Port Detection from Command"
    
    # A package.json dev script that pins the port.
    local dir="$TEST_DIR/port-test"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "port-test-app",
  "scripts": {
    "dev": "vite --port 3001"
  }
}
EOF
    
    # Two cases, because the behaviour differs and only one of them is the
    # thing this suite is really about.
    #
    # An explicit `--port` on the *command Antra runs* wins. Uses a probed
    # free port rather than a literal: 3001 in particular turned out to be
    # held by an unrelated desktop app on this machine, and any hardcoded
    # port is one developer's stray process away from failing the same way.
    free=$(free_port)
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt --port "$free" 2>&1 || true)
    if echo "$output" | grep -q "Using port $free"; then
        log_pass "Explicit --port honored on the command line"
    else
        log_fail "Explicit --port honored on the command line"
    fi
    
    # A `--port` *inside the package.json dev script* is read and wins over
    # the framework default. The argv Antra execs is only `npm run dev`, so
    # the pin has to come out of the script body itself (ROADMAP C9).
    # Asserted as a port choice, not a successful bind: 3001 may be held by
    # an unrelated process, and the run then stops with a
    # port-already-in-use error that still proves 3001 was selected. The
    # 5173 half is the regression guard — if the pin is ever missed again,
    # Vite's default shows up in the output and this fails.
    output=$(run_antra_capped $ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    if echo "$output" | grep -q "3001" && ! echo "$output" | grep -q "5173"; then
        log_pass "Port pinned in the dev script beats the framework default"
    else
        log_fail "Port pinned in the dev script beats the framework default"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# DOMAIN RESOLUTION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_domain_resolution() {
    log_section "Domain Resolution"
    
    # Test .localhost domain
    local dir="$TEST_DIR/domain-test"
    mkdir -p "$dir"
    cd "$dir"
    
    cat > package.json << 'EOF'
{
  "name": "domain-test-app",
  "scripts": {
    "dev": "node -e \"console.log('running')\""
  }
}
EOF
    
    output=$(run_antra_capped $ANTRA_BIN dev --domain custom.localhost --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Domain resolved: custom.localhost"; then
        log_pass ".localhost domain resolution"
    else
        log_fail ".localhost domain resolution"
    fi
    
    # Test a non-`.localhost` TLD. Resolving one means writing /etc/hosts,
    # which needs root — on a developer machine and on a CI runner alike.
    # Check writability up front and SKIP rather than fail: an unprivileged
    # runner is not a broken feature, and the `.localhost` case above has
    # already covered domain resolution itself.
    if [ ! -w /etc/hosts ]; then
        log_skip ".test domain resolution — /etc/hosts is not writable without root"
        return 0
    fi
    
    output=$(run_antra_capped $ANTRA_BIN dev --domain test-app.test --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Domain resolved: test-app.test"; then
        log_pass ".test domain resolution"
    else
        log_fail ".test domain resolution"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# CLEANUP TASKS TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_cleanup_tasks() {
    log_section "Cleanup Tasks Verification"
    
    # C1: Commands2 enum removed
    if ! grep -q "enum Commands2" "$REPO_ROOT/src/cli/mod.rs"; then
        log_pass "C1: Commands2 enum removed"
    else
        log_fail "C1: Commands2 enum removed"
    fi
    
    # C2: #![allow(dead_code)] removed
    if ! grep -q '#!\[allow(dead_code)\]' "$REPO_ROOT/src/main.rs"; then
        log_pass "C2: #![allow(dead_code)] removed"
    else
        log_fail "C2: #![allow(dead_code)] removed"
    fi
    
    # C3: Socket permissions fixed (0o600)
    if grep -q "0o600" "$REPO_ROOT/src/daemon/server.rs"; then
        log_pass "C3: Socket permissions fixed to 0o600"
    else
        log_fail "C3: Socket permissions fixed to 0o600"
    fi
    
    # C4: proxy/server.rs removed
    if [ ! -f "$REPO_ROOT/src/proxy/server.rs" ]; then
        log_pass "C4: proxy/server.rs removed"
    else
        log_fail "C4: proxy/server.rs removed"
    fi
    
    # C5: TTY check added in doctor
    if grep -q "isatty" "$REPO_ROOT/src/cli/doctor.rs"; then
        log_pass "C5: TTY check added in doctor"
    else
        log_fail "C5: TTY check added in doctor"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# SELECT_RESOLVER CONSOLIDATION TEST
# ═══════════════════════════════════════════════════════════════════════════════

test_select_resolver() {
    log_section "select_resolver Consolidation"
    
    # The trailing "(" is load-bearing. `select_resolver_for_registration` is
    # a second, intended function (ROADMAP #5: the only difference is the
    # custom-domain approval check), so a bare `fn select_resolver` matches
    # both, counts 2, and this assertion could never pass. ROADMAP #5 is about
    # the three copy-pasted duplicates in cli/ being gone — which the four
    # `use crate::resolver::util::...` checks below verify — not about the
    # helper existing exactly once.
    count=$(grep -r "fn select_resolver(" "$REPO_ROOT/src/" | wc -l)
    
    if [ "$count" -eq 1 ]; then
        log_pass "select_resolver defined only once"
    else
        log_fail "select_resolver defined only once (found $count)"
    fi
    
    # Check that all CLI files use the shared resolver. `run` and `alias` are
    # registration paths, so they call `select_resolver_for_registration` (the
    # variant that also runs the custom-domain approval check — ROADMAP #5).
    # Matching either name keeps this a test of "the shared helper is used"
    # rather than of which of the two helpers a given call site needs.
    for f in run alias mod; do
        if grep -q "resolver::util::select_resolver" "$REPO_ROOT/src/cli/$f.rs"; then
            log_pass "$f.rs uses shared select_resolver"
        else
            log_fail "$f.rs uses shared select_resolver"
        fi
    done
}

# ═══════════════════════════════════════════════════════════════════════════════
# RUN ALL TESTS
# ═══════════════════════════════════════════════════════════════════════════════

main() {
    echo -e "${BOLD}${CYAN}Starting Antra Comprehensive E2E Tests${RESET}"
    echo -e "${CYAN}$(date)${RESET}\n"
    
    # Build first
    echo -e "${YELLOW}Building antra...${RESET}"
    cargo build --quiet 2>&1 | grep -v "^warning" || true
    echo ""

    # Creates $TEST_DIR and the results header. Without this the scratch
    # dirs were created only as a side effect of the first test's `mkdir -p`
    # and every result was `tee -a`'d onto a file that did not exist.
    setup

    # Node.js tests
    test_node_npm
    test_node_yarn
    test_node_pnpm
    test_node_bun
    test_vite
    test_nextjs
    test_nuxt
    test_react_cra
    test_angular
    test_node_no_scripts
    
    # Rust tests
    test_rust_axum
    test_rust_actix
    test_rust_rocket
    test_rust_no_web
    
    # Go tests
    test_go_gin
    test_go_echo
    test_go_fiber
    test_go_chi
    test_go_no_web
    
    # Python tests
    test_python_fastapi
    test_python_django
    test_python_flask
    test_python_generic
    
    # Ruby tests
    test_ruby_rails
    test_ruby_sinatra
    test_ruby_generic
    
    # Elixir tests
    test_elixir_phoenix
    test_elixir_generic
    
    # PHP tests
    test_php_laravel
    test_php_generic
    
    # Feature tests
    test_env_injection
    test_port_detection
    test_domain_resolution
    
    # Verification tests
    test_cleanup_tasks
    test_select_resolver
    
    # Summary
    log_section "TEST SUMMARY"
    echo -e "${GREEN}Passed: $pass_count${RESET}" | tee -a "$RESULTS_FILE"
    if [ "$fail_count" -gt 0 ]; then
        echo -e "${RED}Failed: $fail_count${RESET}" | tee -a "$RESULTS_FILE"
    else
        echo -e "Failed: 0" | tee -a "$RESULTS_FILE"
    fi
    # Skips are reported, never hidden: a green run with 12 SKIPs says
    # "this machine cannot test yarn, php, or Elixir", which is a different
    # claim from "those features work".
    echo -e "${YELLOW}Skipped: $skip_count${RESET}" | tee -a "$RESULTS_FILE"
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
