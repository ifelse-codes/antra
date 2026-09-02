#!/bin/bash
set -euo pipefail

# Antra — One-line installer
# Usage: curl -fsSL https://antra.iifelse.com/install.sh | bash
#
# This script:
#   1. Detects your OS and architecture
#   2. Downloads the latest Antra binary from GitHub Releases
#   3. Installs it to /usr/local/bin (or ~/.local/bin if unprivileged)
#   4. Optionally installs the local CA into your system trust store

REPO="ifelse-codes/antra"
BINARY_NAME="antra"
GITHUB_BASE_URL="https://github.com/${REPO}/releases/download"
MIN_PORTABLE_DIR="${HOME}/.local/bin"

# ── Colors ────────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

info()  { printf "${CYAN}▸${RESET} %s\n" "$1"; }
ok()    { printf "${GREEN}✓${RESET} %s\n" "$1"; }
warn()  { printf "${YELLOW}⚠${RESET} %s\n" "$1"; }
err()   { printf "${RED}✗${RESET} %s\n" "$1" >&2; }
header(){ printf "\n${BOLD}%s${RESET}\n\n" "$1"; }

# ── Detect platform ───────────────────────────────────────────────────────────

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)  echo "linux";;
        Darwin*) echo "darwin";;
        MINGW*|MSYS*|CYGWIN*) echo "windows";;
        *)
            err "Unsupported OS: $os"
            err "Antra supports macOS, Linux, and Windows."
            exit 1
            ;;
    esac
}

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)   echo "x86_64";;
        arm64|aarch64)   echo "aarch64";;
        *)
            err "Unsupported architecture: $arch"
            err "Antra supports x86_64 and arm64/aarch64."
            exit 1
            ;;
    esac
}

# ── Map to release artifact name ──────────────────────────────────────────────

artifact_name() {
    local os="$1" arch="$2"
    case "${os}-${arch}" in
        darwin-arm64)   echo "antra-aarch64-apple-darwin";;
        darwin-x86_64)  echo "antra-x86_64-apple-darwin";;
        linux-x86_64)   echo "antra-x86_64-linux";;
        linux-arm64)    echo "antra-aarch64-linux";;
        windows-x86_64) echo "antra-x86_64-windows.exe";;
        *)
            err "No release binary for ${os}-${arch}"
            exit 1
            ;;
    esac
}

# ── Get latest version from GitHub ────────────────────────────────────────────

get_latest_version() {
    local version
    version=$(curl -fsSL -o /dev/null -w '%{url_effective}' \
        "https://github.com/${REPO}/releases/latest" 2>/dev/null | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' || true)
    if [ -z "$version" ]; then
        # Fallback: parse from the redirect
        version=$(curl -fsSI "https://github.com/${REPO}/releases/latest" 2>/dev/null \
            | grep -i '^location:' | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' || true)
    fi
    if [ -z "$version" ]; then
        err "Could not determine latest version."
        err "Check https://github.com/${REPO}/releases manually."
        exit 1
    fi
    echo "$version"
}

# ── Download ──────────────────────────────────────────────────────────────────

download_binary() {
    local artifact="$1" version="$2" dest="$3"
    local url="${GITHUB_BASE_URL}/${version}/${artifact}"

    info "Downloading ${artifact} (${version})..."
    if ! curl -fsSL -o "$dest" "$url"; then
        err "Failed to download from $url"
        exit 1
    fi
    chmod +x "$dest"
}

# ── Verify checksum ───────────────────────────────────────────────────────────

verify_checksum() {
    local binary_path="$1" version="$2" artifact="$3"
    local checksum_url="${GITHUB_BASE_URL}/${version}/${artifact}.sha256"
    local expected_hash

    info "Verifying checksum..."
    expected_hash=$(curl -fsSL "$checksum_url" 2>/dev/null | awk '{print $1}' || true)

    if [ -z "$expected_hash" ]; then
        warn "Could not fetch checksum, skipping verification."
        return 0
    fi

    local actual_hash
    if command -v sha256sum &>/dev/null; then
        actual_hash=$(sha256sum "$binary_path" | awk '{print $1}')
    elif command -v shasum &>/dev/null; then
        actual_hash=$(shasum -a 256 "$binary_path" | awk '{print $1}')
    else
        warn "No sha256sum or shasum found, skipping verification."
        return 0
    fi

    if [ "$actual_hash" = "$expected_hash" ]; then
        ok "Checksum verified"
    else
        err "Checksum mismatch!"
        err "  Expected: $expected_hash"
        err "  Got:      $actual_hash"
        exit 1
    fi
}

# ── Install ───────────────────────────────────────────────────────────────────

install_binary() {
    local src="$1"
    local install_dir

    # Try /usr/local/bin first, fall back to ~/.local/bin
    if [ -w /usr/local/bin ] 2>/dev/null; then
        install_dir="/usr/local/bin"
    elif [ -w /usr/local ] 2>/dev/null; then
        install_dir="/usr/local/bin"
    else
        install_dir="$MIN_PORTABLE_DIR"
        mkdir -p "$install_dir"
    fi

    local dest="${install_dir}/${BINARY_NAME}"
    cp "$src" "$dest"
    ok "Installed to ${dest}"

    # Check if install_dir is in PATH
    case ":$PATH:" in
        *":${install_dir}:"*) ;;
        *)
            warn "${install_dir} is not in your PATH."
            warn "Add this to your shell profile:"
            warn ""
            warn "  export PATH=\"${install_dir}:\$PATH\""
            warn ""
            ;;
    esac

    echo "$dest"
}

# ── Ask about trust ───────────────────────────────────────────────────────────

ask_trust() {
    local binary_path="$1"

    header "Trust Setup"
    echo "  Antra generates a local CA certificate to serve HTTPS for"
    echo "  domains like https://myapp.localhost and https://myapp.test."
    echo ""
    echo "  ${BOLD}Installing the CA into your system trust store${RESET} means"
    echo "  HTTPS works with zero browser warnings — forever."
    echo ""
    echo "  ${DIM}This requires admin privileges (sudo) on macOS/Linux.${RESET}"
    echo "  ${DIM}The CA is local-only. Nothing is sent anywhere.${RESET}"
    echo ""
    printf "  Install CA into system trust store? [y/N] "
    read -r response < /dev/tty

    case "$response" in
        [yY][eE][sS]|[yY])
            echo ""
            info "Installing CA into system trust store..."
            if "$binary_path" trust; then
                ok "CA installed. HTTPS will work with no warnings."
            else
                echo ""
                warn "CA install failed or was cancelled."
                warn "You can run 'antra trust' later to try again."
                warn "You can run 'antra doctor' to diagnose issues."
            fi
            ;;
        *)
            echo ""
            warn "Skipped. You can run 'antra trust' later."
            warn "HTTPS for custom domains may show cert warnings until then."
            warn "You can run 'antra doctor' to check your setup anytime."
            ;;
    esac
}

# ── Main ──────────────────────────────────────────────────────────────────────

main() {
    header "ANTRA INSTALLER"

    local os arch artifact version tmp_dir tmp_binary installed_path

    os="$(detect_os)"
    arch="$(detect_arch)"
    artifact="$(artifact_name "$os" "$arch")"

    info "Detected: ${os}/${arch}"

    version="$(get_latest_version)"
    info "Latest version: ${version}"

    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    tmp_binary="${tmp_dir}/${BINARY_NAME}"
    download_binary "$artifact" "$version" "$tmp_binary"
    verify_checksum "$tmp_binary" "$version" "$artifact"

    echo ""
    installed_path="$(install_binary "$tmp_binary")"

    echo ""
    ok "Antra $( "$installed_path" --version 2>/dev/null || echo "${version}" ) is ready!"
    echo ""
    echo "  ${BOLD}Quick start:${RESET}"
    echo ""
    echo "    antra run --domain myapp.localhost -- pnpm dev"
    echo ""
    echo "  ${DIM}Open https://myapp.localhost in your browser. Done.${RESET}"
    echo ""

    ask_trust "$installed_path"

    header "NEXT STEPS"
    echo "  1. Run ${BOLD}antra doctor${RESET} to verify everything works"
    echo "  2. Run ${BOLD}antra run --domain myapp.localhost -- <your-dev-command>${RESET}"
    echo "  3. Open ${BOLD}https://myapp.localhost${RESET}"
    echo ""
    echo "  ${DIM}Docs: https://github.com/${REPO}${RESET}"
    echo ""
}

main "$@"
