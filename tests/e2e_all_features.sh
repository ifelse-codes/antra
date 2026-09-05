#!/bin/bash
set -e

# Antra Comprehensive E2E Test Script
# Tests all supported languages/frameworks and features

ANTRA_BIN="./target/debug/antra"
TEST_DIR="/tmp/antra-e2e-tests"
RESULTS_FILE="/tmp/antra-test-results.txt"

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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "pnpm run dev"; then
        log_pass "pnpm command inferred correctly"
    else
        log_fail "pnpm command inferred correctly"
    fi
}

test_node_bun() {
    log_section "Node.js (bun)"
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Rust project: test-axum-app"; then
        log_pass "Rust project detected"
    else
        log_fail "Rust project detected"
    fi
    
    if echo "$output" | grep -q "cargo run"; then
        log_pass "cargo run command used"
    else
        log_fail "cargo run command used"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:8080"; then
        log_pass "Default port 8080 for axum"
    else
        log_fail "Default port 8080 for axum"
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Go project: test-gin-app"; then
        log_pass "Go project detected"
    else
        log_fail "Go project detected"
    fi
    
    if echo "$output" | grep -q "go run"; then
        log_pass "go run command used"
    else
        log_fail "go run command used"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:8080"; then
        log_pass "Default port 8080 for Go"
    else
        log_fail "Default port 8080 for Go"
    fi
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Python project: test-fastapi-app"; then
        log_pass "Python project detected"
    else
        log_fail "Python project detected"
    fi
    
    if echo "$output" | grep -q "uvicorn"; then
        log_pass "uvicorn command used for FastAPI"
    else
        log_fail "uvicorn command used for FastAPI"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:8000"; then
        log_pass "Default port 8000 for FastAPI"
    else
        log_fail "Default port 8000 for FastAPI"
    fi
}

test_python_django() {
    log_section "Python (Django)"
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Python project: test-flask-app"; then
        log_pass "Python Flask project detected"
    else
        log_fail "Python Flask project detected"
    fi
    
    if echo "$output" | grep -q "flask run"; then
        log_pass "Flask command used"
    else
        log_fail "Flask command used"
    fi
}

test_python_generic() {
    log_section "Python (generic)"
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Ruby on Rails project"; then
        log_pass "Ruby Rails project detected"
    else
        log_fail "Ruby Rails project detected"
    fi
    
    if echo "$output" | grep -q "bundle exec rails server"; then
        log_pass "Rails server command used"
    else
        log_fail "Rails server command used"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:3000"; then
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Ruby (Sinatra) project"; then
        log_pass "Ruby Sinatra project detected"
    else
        log_fail "Ruby Sinatra project detected"
    fi
    
    if echo "$output" | grep -q "127.0.0.1:4567"; then
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Detected Ruby project"; then
        log_pass "Ruby generic project detected"
    else
        log_fail "Ruby generic project detected"
    fi
    
    if echo "$output" | grep -q "bundle exec rackup"; then
        log_pass "Rackup command used"
    else
        log_fail "Rackup command used"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# ELIXIR TESTS
# ═══════════════════════════════════════════════════════════════════════════════

test_elixir_phoenix() {
    log_section "Elixir (Phoenix)"
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
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
    
    # Test --port flag
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
    
    output=$($ANTRA_BIN dev --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "detected port 3001"; then
        log_pass "Port detected from --port flag"
    else
        log_fail "Port detected from --port flag"
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
    
    output=$($ANTRA_BIN dev --domain custom.localhost --no-trust-prompt 2>&1 || true)
    
    if echo "$output" | grep -q "Domain resolved: custom.localhost"; then
        log_pass ".localhost domain resolution"
    else
        log_fail ".localhost domain resolution"
    fi
    
    # Test .test domain
    output=$($ANTRA_BIN dev --domain test-app.test --no-trust-prompt 2>&1 || true)
    
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
    if ! grep -q "enum Commands2" src/cli/mod.rs; then
        log_pass "C1: Commands2 enum removed"
    else
        log_fail "C1: Commands2 enum removed"
    fi
    
    # C2: #![allow(dead_code)] removed
    if ! grep -q '#!\[allow(dead_code)\]' src/main.rs; then
        log_pass "C2: #![allow(dead_code)] removed"
    else
        log_fail "C2: #![allow(dead_code)] removed"
    fi
    
    # C3: Socket permissions fixed (0o600)
    if grep -q "0o600" src/daemon/server.rs; then
        log_pass "C3: Socket permissions fixed to 0o600"
    else
        log_fail "C3: Socket permissions fixed to 0o600"
    fi
    
    # C4: proxy/server.rs removed
    if [ ! -f src/proxy/server.rs ]; then
        log_pass "C4: proxy/server.rs removed"
    else
        log_fail "C4: proxy/server.rs removed"
    fi
    
    # C5: TTY check added in doctor
    if grep -q "isatty" src/cli/doctor.rs; then
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
    
    # Check that select_resolver is only defined once in resolver/util.rs
    count=$(grep -r "fn select_resolver" src/ | wc -l)
    
    if [ "$count" -eq 1 ]; then
        log_pass "select_resolver defined only once"
    else
        log_fail "select_resolver defined only once (found $count)"
    fi
    
    # Check that all CLI files use the shared resolver
    if grep -q "use crate::resolver::util::select_resolver" src/cli/run.rs; then
        log_pass "run.rs uses shared select_resolver"
    else
        log_fail "run.rs uses shared select_resolver"
    fi
    
    if grep -q "use crate::resolver::util::select_resolver" src/cli/alias.rs; then
        log_pass "alias.rs uses shared select_resolver"
    else
        log_fail "alias.rs uses shared select_resolver"
    fi
    
    if grep -q "use crate::resolver::util::select_resolver" src/cli/mod.rs; then
        log_pass "mod.rs uses shared select_resolver"
    else
        log_fail "mod.rs uses shared select_resolver"
    fi
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
