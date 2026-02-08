#!/usr/bin/env bash
# EchoVault CLI Install Script (Linux/macOS)
# Installs echovault-cli to ~/.local/bin (globally accessible)
#
# Usage: curl -fsSL https://raw.githubusercontent.com/n24q02m/EchoVault/main/install-cli.sh | bash
#
# Environment variables:
#   VERSION     - Install specific version (e.g., "1.17.0"), default: latest
#   INSTALL_DIR - Custom install directory (default: ~/.local/bin)
#   DRY_RUN     - Set to "1" to print commands without executing

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

REPO="n24q02m/EchoVault"
BIN_NAME="echovault-cli"
GITHUB_API="https://api.github.com/repos/${REPO}/releases"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

run() {
    if [[ "${DRY_RUN:-0}" == "1" ]]; then
        echo -e "${YELLOW}[DRY-RUN]${NC} $*"
    else
        "$@"
    fi
}

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  PLATFORM="linux" ;;
        Darwin) PLATFORM="macos" ;;
        *)      error "Unsupported OS: $OS. Use install-cli.ps1 for Windows." ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH_SUFFIX="x64" ;;
        aarch64|arm64)  ARCH_SUFFIX="arm64" ;;
        *)              error "Unsupported architecture: $ARCH" ;;
    esac

    # macOS arm64 support; Linux only x64 currently
    if [[ "$PLATFORM" == "linux" && "$ARCH_SUFFIX" == "arm64" ]]; then
        error "Linux arm64 is not yet supported. Use x64 or build from source."
    fi

    ARTIFACT_NAME="${BIN_NAME}-${PLATFORM}-${ARCH_SUFFIX}"
    info "Detected: $PLATFORM ($ARCH_SUFFIX)"
}

get_version() {
    if [[ -n "${VERSION:-}" ]]; then
        RELEASE_VERSION="$VERSION"
        info "Using specified version: v$RELEASE_VERSION"
        return
    fi

    info "Fetching latest version..."

    local response
    response=$(curl -fsSL -H "User-Agent: EchoVault-CLI-Installer" "${GITHUB_API}/latest" 2>/dev/null) && {
        RELEASE_VERSION=$(echo "$response" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')
        if [[ -n "$RELEASE_VERSION" ]]; then
            info "Latest version: v$RELEASE_VERSION"
            return
        fi
    }

    # Fallback: redirect URL
    info "API rate limited, using fallback..."
    local redirect_url
    redirect_url=$(curl -fsSI "https://github.com/${REPO}/releases/latest" 2>/dev/null | grep -i "^location:" | tr -d '\r' | awk '{print $2}')

    if [[ -n "$redirect_url" ]]; then
        RELEASE_VERSION=$(echo "$redirect_url" | sed -E 's|.*/tag/v||')
    fi

    if [[ -z "${RELEASE_VERSION:-}" ]]; then
        error "Failed to fetch latest version. Try: VERSION=x.x.x $0"
    fi

    info "Latest version: v$RELEASE_VERSION"
}

download_and_install() {
    local url="https://github.com/${REPO}/releases/download/v${RELEASE_VERSION}/${ARTIFACT_NAME}"
    local dest="${INSTALL_DIR}/${BIN_NAME}"

    info "Downloading ${BIN_NAME} v${RELEASE_VERSION}..."

    run mkdir -p "$INSTALL_DIR"

    if [[ "${DRY_RUN:-0}" == "1" ]]; then
        run curl -fSL --progress-bar -o "$dest" "$url"
        run chmod +x "$dest"
    else
        curl -fSL --progress-bar -o "$dest" "$url"
        if [[ ! -f "$dest" ]]; then
            error "Download failed. URL: $url"
        fi
        chmod +x "$dest"
    fi

    success "Installed to $dest"
}

ensure_in_path() {
    if [[ ":$PATH:" == *":${INSTALL_DIR}:"* ]]; then
        return
    fi

    warn "${INSTALL_DIR} is not in your PATH"

    # Detect shell and rc file
    local shell_name rc_file
    shell_name="$(basename "${SHELL:-/bin/bash}")"
    case "$shell_name" in
        zsh)  rc_file="$HOME/.zshrc" ;;
        fish) rc_file="$HOME/.config/fish/config.fish" ;;
        *)    rc_file="$HOME/.bashrc" ;;
    esac

    local export_line="export PATH=\"${INSTALL_DIR}:\$PATH\""
    if [[ "$shell_name" == "fish" ]]; then
        export_line="fish_add_path ${INSTALL_DIR}"
    fi

    # Check if already in rc file
    if [[ -f "$rc_file" ]] && grep -qF "$INSTALL_DIR" "$rc_file" 2>/dev/null; then
        info "PATH entry already in $rc_file (restart shell to apply)"
        return
    fi

    info "Adding to $rc_file..."
    run echo "$export_line" >> "$rc_file"
    success "Added PATH entry to $rc_file"
    warn "Run: source $rc_file  (or restart your terminal)"
}

verify_install() {
    local dest="${INSTALL_DIR}/${BIN_NAME}"

    if [[ "${DRY_RUN:-0}" == "1" ]]; then
        return
    fi

    if [[ -x "$dest" ]]; then
        success "Verification: $dest is executable"
    else
        error "Verification failed: $dest is not executable"
    fi
}

main() {
    for arg in "$@"; do
        case "$arg" in
            --help|-h)
                cat << 'EOF'
EchoVault CLI Install Script

Usage:
    curl -fsSL https://raw.githubusercontent.com/n24q02m/EchoVault/main/install-cli.sh | bash

Environment Variables:
    VERSION      Specific version to install (default: latest)
    INSTALL_DIR  Installation directory (default: ~/.local/bin)
    DRY_RUN      Set to "1" to preview commands

After install, use as MCP server:
    echovault-cli mcp
EOF
                exit 0 ;;
        esac
    done

    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}    EchoVault CLI Installer${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""

    detect_platform
    get_version
    download_and_install
    ensure_in_path
    verify_install

    echo ""
    success "Installation complete!"
    echo ""
    info "Quick start:"
    echo "  echovault-cli extract    # Extract sessions from IDEs"
    echo "  echovault-cli parse      # Parse into Markdown"
    echo "  echovault-cli embed      # Build search index"
    echo "  echovault-cli mcp        # Start MCP server"
    echo ""
    info "MCP config (Claude Desktop, Copilot, Cursor):"
    echo '  { "command": "echovault-cli", "args": ["mcp"] }'
    echo ""
}

main "$@"
